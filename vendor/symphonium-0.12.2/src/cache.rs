#[cfg(feature = "resampler")]
use crate::resample::ResamplerKey;
#[cfg(feature = "resampler")]
use fixed_resample::{PacketResampler, Sequential};
#[cfg(feature = "resampler")]
use std::collections::HashMap;
use symphonia::core::codecs::{
    audio::{AudioDecoder, AudioDecoderOptions},
    registry::CodecRegistry,
};

#[cfg(feature = "resampler")]
use std::cell::RefCell;
#[cfg(all(feature = "resampler", feature = "multithreaded-cache"))]
use std::sync::{Arc, Mutex, RwLock};

use crate::{ProbedAudioSource, create_decoder, error::LoadError};

/// A cache of audio file decoders and resamplers for symphonium.
pub trait Cache {
    fn with_decoder_mut(
        &self,
        probed: ProbedAudioSource,
        options: &AudioDecoderOptions,
        codec_registry: &CodecRegistry,
        track_index: Option<usize>,
        f: &mut dyn FnMut(ProbedAudioSource, &mut dyn AudioDecoder),
    ) -> Result<(), LoadError>;

    #[allow(clippy::type_complexity)]
    #[cfg(feature = "resampler")]
    fn with_resampler_mut(
        &self,
        key: ResamplerKey,
        probed: ProbedAudioSource,
        f: &mut dyn FnMut(ProbedAudioSource, &mut PacketResampler<f32, Sequential<f32>>),
    );
}

/// A default single-threaded implementation for a [`Cache`].
///
/// Note, this currently only caches resamplers. Caching decoders has not
/// been implemented yet.
///
/// If the `multithreaded-cache` feature is enabled, then this struct can be
/// cloned and shared with other threads. Otherwise, this struct is `!Clone`
/// and `!Sync`
pub struct SymphoniumCache {
    #[cfg(feature = "resampler")]
    resampler_cache: RefCell<ResamplerHashMap>,
}

impl SymphoniumCache {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "resampler")]
            resampler_cache: RefCell::new(ResamplerHashMap::default()),
        }
    }

    pub fn clear_cache(&mut self) {
        #[cfg(feature = "resampler")]
        if let Ok(mut resampler_cache) = self.resampler_cache.try_borrow_mut() {
            resampler_cache.clear();
            resampler_cache.shrink_to_fit();
        }
    }
}

impl Cache for SymphoniumCache {
    fn with_decoder_mut(
        &self,
        probed: ProbedAudioSource,
        options: &AudioDecoderOptions,
        codec_registry: &CodecRegistry,
        track_index: Option<usize>,
        f: &mut dyn FnMut(ProbedAudioSource, &mut dyn AudioDecoder),
    ) -> Result<(), LoadError> {
        // TODO: Add decoder caching?
        let mut decoder = create_decoder(&probed, track_index, options, codec_registry)?;

        (f)(probed, &mut *decoder);

        Ok(())
    }

    #[cfg(feature = "resampler")]
    fn with_resampler_mut(
        &self,
        key: ResamplerKey,
        probed: ProbedAudioSource,
        f: &mut dyn FnMut(ProbedAudioSource, &mut PacketResampler<f32, Sequential<f32>>),
    ) {
        if let Ok(mut resampler_cache) = self.resampler_cache.try_borrow_mut() {
            let resampler = resampler_cache
                .entry(key)
                .or_insert_with(|| key.create_resampler());

            (f)(probed, resampler);
            return;
        }

        // Fallback to temporary resampler.
        let mut resampler = key.create_resampler();
        (f)(probed, &mut resampler);
    }
}

impl Default for SymphoniumCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "resampler")]
type ResamplerHashMap = HashMap<ResamplerKey, PacketResampler<f32, Sequential<f32>>>;

// ---------------------------------------------------------------------------------------

/// A default multi-threaded implementation for a [`Cache`].
///
/// Note, this currently only caches resamplers. Caching decoders has not
/// been implemented yet.
///
/// This struct can be wrapped inside of an `Arc<SyncSymphoniumCache>` and
/// shared with other threads.
#[cfg(feature = "multithreaded-cache")]
pub struct SyncSymphoniumCache {
    #[cfg(feature = "resampler")]
    resampler_cache: RwLock<SyncResamplerHashMap>,
    #[cfg(feature = "resampler")]
    max_num_concurrent_caches: u16,
}

#[cfg(feature = "multithreaded-cache")]
impl SyncSymphoniumCache {
    pub fn with_max_num_concurrent_caches(max_num_concurrent_caches: u16) -> Self {
        #[cfg(not(feature = "resampler"))]
        let _ = max_num_concurrent_caches;

        Self {
            #[cfg(feature = "resampler")]
            resampler_cache: RwLock::new(SyncResamplerHashMap::default()),
            #[cfg(feature = "resampler")]
            max_num_concurrent_caches,
        }
    }

    pub fn clear_cache(&mut self) {
        #[cfg(feature = "resampler")]
        if let Ok(mut resampler_cache) = self.resampler_cache.write() {
            resampler_cache.clear();
            resampler_cache.shrink_to_fit();
        }
    }
}

#[cfg(feature = "multithreaded-cache")]
impl Cache for SyncSymphoniumCache {
    fn with_decoder_mut(
        &self,
        probed: ProbedAudioSource,
        options: &AudioDecoderOptions,
        codec_registry: &CodecRegistry,
        track_index: Option<usize>,
        f: &mut dyn FnMut(ProbedAudioSource, &mut dyn AudioDecoder),
    ) -> Result<(), LoadError> {
        // TODO: Add decoder caching?
        let mut decoder = create_decoder(&probed, track_index, options, codec_registry)?;

        (f)(probed, &mut *decoder);

        Ok(())
    }

    #[cfg(feature = "resampler")]
    fn with_resampler_mut(
        &self,
        key: ResamplerKey,
        probed: ProbedAudioSource,
        f: &mut dyn FnMut(ProbedAudioSource, &mut PacketResampler<f32, Sequential<f32>>),
    ) {
        {
            let mut new_resampler_key = None;

            if let Ok(resampler_cache) = self.resampler_cache.read() {
                for i in 0..self.max_num_concurrent_caches {
                    let sync_key = SyncResamplerKey { key, instance: i };

                    let resampler_entry = if let Some(entry) = resampler_cache.get(&sync_key) {
                        Arc::clone(entry)
                    } else {
                        new_resampler_key = Some(sync_key);
                        break;
                    };

                    if let Ok(mut resampler) = resampler_entry.try_lock() {
                        // Drop the outer mutex so that other threads can access the hashmap.
                        drop(resampler_cache);

                        (f)(probed, &mut resampler);
                        return;
                    }
                }
            }

            if let Some(key) = new_resampler_key {
                let mut new_resampler = key.key.create_resampler();
                (f)(probed, &mut new_resampler);

                let Ok(mut resampler_cache) = self.resampler_cache.write() else {
                    return;
                };

                resampler_cache.insert(key, Arc::new(Mutex::new(new_resampler)));
                return;
            }
        }

        // Fallback to temporary resampler.
        let mut resampler = key.create_resampler();
        (f)(probed, &mut resampler);
    }
}

#[cfg(feature = "multithreaded-cache")]
impl Default for SyncSymphoniumCache {
    fn default() -> Self {
        Self::with_max_num_concurrent_caches(3)
    }
}

#[cfg(all(feature = "resampler", feature = "multithreaded-cache"))]
type SyncResamplerHashMap =
    HashMap<SyncResamplerKey, Arc<Mutex<PacketResampler<f32, Sequential<f32>>>>>;

#[cfg(all(feature = "resampler", feature = "multithreaded-cache"))]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct SyncResamplerKey {
    key: ResamplerKey,
    instance: u16,
}
