//! An unofficial easy-to-use wrapper around [Symphonia](https://github.com/pdeljanov/Symphonia)
//! for loading audio files. It also handles resampling at load-time.
//!
//! The resulting `DecodedAudio` resources are stored in their native sample format whenever
//! possible to save on memory, and have convenience methods to fill a buffer with `f32` samples
//! from any arbitrary position in the resource in realtime during playback. Alternatively you
//! can use the `DecodedAudioF32` resource if you only need samples in the `f32` format.
//!
//! ## Example
//!
//! ```no_run
//! // A struct used to load audio files.
//! # use std::num::NonZeroU32;
//! # use symphonium::{DecodeConfig, cache::SymphoniumCache};
//! # let file_path = std::path::PathBuf::default();
//! let target_sample_rate = NonZeroU32::new(44100).unwrap();
//!
//! // An optional cache to re-use decoders and resamplers.
//! let cache = SymphoniumCache::default();
//!
//! // Probe the audio file.
//! let probed = symphonium::probe_from_file(
//!     &file_path,
//!     // A custom codec prober. Set to `None` to use the default one from symphonia.
//!     None,
//! )
//! .unwrap();
//!
//! // Decode the probed data.
//! let audio_data = symphonium::decode(
//!     probed,
//!     &DecodeConfig::default(),
//!     // Set to `None` to keep the original sample rate of the file.
//!     Some(target_sample_rate),
//!     // Set to `None` if no cache is needed.
//!     Some(&cache),
//!     // A custom codec registry. Set to `None` to use the default one from symphonia.
//!     None,
//! )
//! .unwrap();
//!
//! // Fill a stereo buffer with samples starting at frame 100.
//! let mut buf_l = vec![0.0f32; 512];
//! let mut buf_r = vec![0.0f32; 512];
//! audio_data.fill_stereo(100, &mut buf_l, &mut buf_r);
//!
//! // Alternatively, if you don't need to save memory, you can
//! // decode directly to an `f32` format.
//! let probed = symphonium::probe_from_file(&file_path, None).unwrap();
//! let audio_data_f32 = symphonium::decode_f32(
//!         probed,
//!         &DecodeConfig::default(),
//!         Some(target_sample_rate),
//!         Some(&cache),
//!         None,
//!     )
//!     .unwrap();
//!
//! // Print info about the data (`data` is a `Vec<Vec<f32>>`).
//! println!("num channels: {}", audio_data_f32.data.len());
//! println!("num frames: {}", audio_data_f32.data[0].len());
//! ```
//! ## Features
//!
//! By default, only `wav` and `ogg` support is enabled. If you need more formats, enable them
//! as features in your `Cargo.toml` file like this:
//!
//! `symphonium = { version = "0.11", features = ["mp3", "flac"] }`
//!
//! Available codecs:
//!
//! * `aac`
//! * `adpcm`
//! * `alac`
//! * `flac`
//! * `mp1`
//! * `mp2`
//! * `mp3`
//! * `pcm`
//! * `vorbis`
//!
//! Available container formats:
//!
//! * `caf`
//! * `isomp4`
//! * `mkv`
//! * `ogg`
//! * `aiff`
//! * `wav`
//!
//! Alternatively you can enable the `all` feature if you want everything, or the `open-standards`
//! feature if you want all of the royalty-free open-source standards.

#![forbid(unsafe_code)]
#![deny(clippy::arithmetic_side_effects)]

use std::fs::File;
use std::num::{NonZeroU32, NonZeroUsize};
use std::path::Path;

#[cfg(feature = "stretch-sinc-resampler")]
use fixed_resample::{PacketResampler, rubato};

use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::codecs::registry::CodecRegistry;
use symphonia::core::formats::probe::{Hint, Probe};
use symphonia::core::formats::{FormatOptions, FormatReader, Track, TrackType};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;

#[cfg(all(feature = "log", not(feature = "tracing")))]
use log::warn;
#[cfg(feature = "tracing")]
use tracing::warn;

// Re-export symphonia
pub use symphonia;

#[cfg(feature = "resampler")]
pub mod resample;
#[cfg(feature = "resampler")]
pub use resample::ResampleQuality;

pub mod cache;
pub mod error;

mod decode;
mod resource;

pub use resource::*;

use error::LoadError;

use crate::cache::Cache;
#[cfg(feature = "resampler")]
use crate::resample::ResamplerKey;

/// The default maximum size of an audio file in bytes.
pub const DEFAULT_MAX_BYTES: usize = 1_000_000_000;

/// Load an audio file from the given path and probe its metadata.
///
/// This method does not decode the audio file.
///
/// * `path` - The path to the audio file.
/// * `custom_probe` - The custom [`Probe`] to use. If `None`, then the default symphonia
///   probe will be used.
pub fn probe_from_file<P: AsRef<Path>>(
    path: P,
    custom_probe: Option<&Probe>,
) -> Result<ProbedAudioSource, LoadError> {
    let path: &Path = path.as_ref();

    // Try to open the file.
    let file = File::open(path)?;

    // Create a hint to help the format registry guess what format reader is appropriate.
    let mut hint = Hint::new();

    // Provide the file extension as a hint.
    if let Some(extension) = path.extension()
        && let Some(extension_str) = extension.to_str()
    {
        hint.with_extension(extension_str);
    }

    probe_from_source(Box::new(file), Some(hint), custom_probe)
}

/// Load an audio source from RAM and probe its metadata.
///
/// This method does not decode the audio file.
///
/// * `source` - The [`MediaSource`] to probe.
/// * `hint` - An optional hint that the probe can use to determine the codec.
/// * `custom_probe` - The custom [`Probe`] to use. If `None`, then the default symphonia
///   probe will be used.
pub fn probe_from_source(
    source: Box<dyn MediaSource>,
    hint: Option<Hint>,
    custom_probe: Option<&Probe>,
) -> Result<ProbedAudioSource, LoadError> {
    let probe = custom_probe.unwrap_or_else(|| symphonia::default::get_probe());

    // Create the media source stream.
    let mss = MediaSourceStream::new(source, Default::default());

    // Use the default options for format reader, metadata reader, and decoder.
    let format_opts: FormatOptions = Default::default();
    let metadata_opts: MetadataOptions = Default::default();

    let hint = hint.unwrap_or_default();

    // Probe the media source stream for metadata and get the format reader.
    let format_reader = probe
        .probe(&hint, mss, format_opts, metadata_opts)
        .map_err(LoadError::UnkownFormat)?;

    // Get the default track in the audio stream.
    let track = format_reader
        .default_track(TrackType::Audio)
        .ok_or(LoadError::NoAudioTrackFound)?;

    let audio_params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or(LoadError::NoAudioCodecFound)?;

    let num_channels = if let Some(ch) = audio_params.channels.as_ref().map(|ch| ch.count())
        && ch > 0
    {
        ch
    } else {
        return Err(LoadError::NoAudioChannelsFound);
    };

    let sample_rate = if let Some(sr) = audio_params.sample_rate {
        let sr = NonZeroU32::new(sr);
        #[cfg(any(feature = "tracing", feature = "log"))]
        {
            if sr.is_none() {
                warn!("MediaSource returned a sample rate of 0");
            }
        }
        sr
    } else {
        None
    };

    Ok(ProbedAudioSource {
        format_reader,
        sample_rate,
        num_channels: NonZeroUsize::new(num_channels).unwrap(),
    })
}

/// Decode a probed audio source, and resample if the source sample rate does not match
/// the given target sample rate.
///
/// The probed audio source can be loaded with [`probe_from_file`] or
/// [`probe_from_source`].
///
/// * `probed` - The probed audio source.
/// * `config` - Additional decoding settings like resampling quality.
/// * `target_sample_rate` - The sample rate the file will be resampled to. (No
///   resampling will occur if the audio file's sample rate is already
///   the target sample rate).
///     * If this is `None`, or if the `resample` feature is disabled, then the
///       file will not be resampled.
///     * Resampling will always convert the sample format to `f32`.
/// * `cache` - An optional cache to use. You can use a [`Cache`]
///   instance. If `None`, then no caching will occur.
/// * `custom_registry` - The custom [`CodecRegistry`] to use. If `None`, then the default
///   symphonia codec registry will be used.
#[cfg(feature = "decode-native")]
pub fn decode(
    probed: ProbedAudioSource,
    config: &DecodeConfig,
    target_sample_rate: Option<NonZeroU32>,
    cache: Option<&dyn Cache>,
    custom_registry: Option<&CodecRegistry>,
) -> Result<DecodedAudio, LoadError> {
    #[cfg(not(feature = "resampler"))]
    let _ = target_sample_rate;

    let mut pcm = None;

    with_decoder(
        probed,
        config,
        custom_registry,
        cache,
        |mut probed: ProbedAudioSource,
         decoder: &mut dyn AudioDecoder,
         original_sample_rate: NonZeroU32| {
            #[cfg(feature = "resampler")]
            if let Some(target_sample_rate) = target_sample_rate
                && original_sample_rate != target_sample_rate
            {
                // Resampling is needed.
                pcm = Some(
                    resample(
                        probed,
                        config,
                        target_sample_rate,
                        original_sample_rate,
                        cache,
                        decoder,
                    )
                    .map(|pcm| pcm.into()),
                );

                return;
            }

            pcm = Some(decode::decode_native_bitdepth(
                probed.format_reader.as_mut(),
                config,
                probed.num_channels,
                original_sample_rate,
                original_sample_rate,
                decoder,
            ));
        },
    )?;

    pcm.unwrap()
}

/// Decode the probed audio source and convert to an f32 sample format.
///
/// The probed audio source can be loaded with [`probe_from_file`] or
/// [`probe_from_source`].
///
/// * `probed` - The probed audio source.
/// * `config` - Additional decoding settings like resampling quality.
/// * `target_sample_rate` - The sample rate the file will be resampled to. (No
///   resampling will occur if the audio file's sample rate is already
///   the target sample rate).
///     * If this is `None`, or if the `resample` feature is disabled, then the
///       file will not be resampled.
/// * `cache` - An optional cache to use. You can use a [`Cache`]
///   instance. If `None`, then no caching will occur.
/// * `custom_registry` - The custom [`CodecRegistry`] to use. If `None`, then the default
///   symphonia codec registry will be used.
pub fn decode_f32(
    probed: ProbedAudioSource,
    config: &DecodeConfig,
    target_sample_rate: Option<NonZeroU32>,
    cache: Option<&dyn Cache>,
    custom_registry: Option<&CodecRegistry>,
) -> Result<DecodedAudioF32, LoadError> {
    #[cfg(not(feature = "resampler"))]
    let _ = target_sample_rate;

    let mut pcm = None;

    with_decoder(
        probed,
        config,
        custom_registry,
        cache,
        |mut probed: ProbedAudioSource,
         decoder: &mut dyn AudioDecoder,
         original_sample_rate: NonZeroU32| {
            #[cfg(feature = "resampler")]
            if let Some(target_sample_rate) = target_sample_rate
                && original_sample_rate != target_sample_rate
            {
                // Resampling is needed.
                pcm = Some(resample(
                    probed,
                    config,
                    target_sample_rate,
                    original_sample_rate,
                    cache,
                    decoder,
                ));

                return;
            }

            pcm = Some(decode::decode_f32(
                probed.format_reader.as_mut(),
                config,
                probed.num_channels,
                original_sample_rate,
                original_sample_rate,
                decoder,
            ));
        },
    )?;

    pcm.unwrap()
}

/// Decode the probed audio source and convert to an f32 sample format. The sample will
/// be stretched (pitch/time shifted) by the given amount.
///
/// The probed audio source can be loaded with [`probe_from_file`] or
/// [`probe_from_source`].
///
/// * `probed` - The probed audio source.
/// * `stretch` - The amount of stretching (`new_length / old_length`). A value of `1.0` is no
///   change, a value less than `1.0` will increase the pitch & decrease the length, and a value
///   greater than `1.0` will decrease the pitch & increase the length. If a `target_sample_rate`
///   is given, then the final amount will automatically be adjusted to account for that.
/// * `target_sample_rate` - If this is `Some`, then the file will be resampled to that
///   sample rate. If this is `None`, then the file will not be resampled and it will stay its
///   original sample rate.
/// * `config` - Extra configuration.
/// * `cache` - An optional cache to use. You can use a [`Cache`]
///   instance. If `None`, then no caching will occur.
/// * `custom_registry` - The custom [`CodecRegistry`] to use. If `None`, then the default
///   symphonia codec registry will be used.
#[cfg(feature = "stretch-sinc-resampler")]
pub fn decode_stretched(
    probed: ProbedAudioSource,
    stretch: f64,
    target_sample_rate: Option<NonZeroU32>,
    config: &DecodeStretchedConfig,
    cache: Option<&dyn Cache>,
    custom_registry: Option<&CodecRegistry>,
) -> Result<DecodedAudioF32, LoadError> {
    use fixed_resample::rubato;

    let mut pcm = None;

    with_decoder(
        probed,
        &config.config,
        custom_registry,
        cache,
        |mut probed: ProbedAudioSource,
         decoder: &mut dyn AudioDecoder,
         original_sample_rate: NonZeroU32| {
            let mut needs_resample = stretch != 1.0;
            if let Some(target_sample_rate) = target_sample_rate
                && !needs_resample
            {
                needs_resample = original_sample_rate != target_sample_rate;
            }

            pcm = if needs_resample {
                let out_sample_rate = target_sample_rate.unwrap_or(original_sample_rate);
                let ratio =
                    (out_sample_rate.get() as f64 / original_sample_rate.get() as f64) * stretch;

                let mut resampler = PacketResampler::from_custom(Box::new(
                    rubato::Async::new_sinc(
                        ratio,
                        1.0,
                        &config.resampler_config,
                        512,
                        probed.num_channels.get(),
                        rubato::FixedAsync::Input,
                    )
                    .unwrap(),
                ));

                Some(decode::decode_resampled(
                    probed.format_reader.as_mut(),
                    &config.config,
                    out_sample_rate,
                    original_sample_rate,
                    probed.num_channels,
                    &mut resampler,
                    decoder,
                ))
            } else {
                Some(decode::decode_f32(
                    probed.format_reader.as_mut(),
                    &config.config,
                    probed.num_channels,
                    original_sample_rate,
                    original_sample_rate,
                    decoder,
                ))
            };
        },
    )?;

    pcm.unwrap()
}

fn with_decoder(
    probed: ProbedAudioSource,
    config: &DecodeConfig,
    custom_registry: Option<&CodecRegistry>,
    cache: Option<&dyn Cache>,
    mut f: impl FnMut(ProbedAudioSource, &mut dyn AudioDecoder, NonZeroU32),
) -> Result<(), LoadError> {
    let original_sample_rate = probed.sample_rate.unwrap_or_else(|| {
        #[cfg(any(feature = "tracing", feature = "log"))]
        warn!("Audio resource has an unkown sample rate. Assuming a sample rate of 44100...");
        NonZeroU32::new(44100).unwrap()
    });

    let codec_registry = custom_registry.unwrap_or_else(|| symphonia::default::get_codecs());

    let options = AudioDecoderOptions::default()
        .verify(config.verify)
        .gapless(config.gapless);

    if let Some(cache) = cache
        && config.cache_decoder
    {
        cache.with_decoder_mut(
            probed,
            &options,
            codec_registry,
            config.track_index,
            &mut |probed, decoder| (f)(probed, decoder, original_sample_rate),
        )?;
    } else {
        let mut decoder = create_decoder(&probed, config.track_index, &options, codec_registry)?;

        (f)(probed, &mut *decoder, original_sample_rate);
    }

    Ok(())
}

#[cfg(feature = "resampler")]
fn resample(
    mut probed: ProbedAudioSource,
    config: &DecodeConfig,
    target_sample_rate: NonZeroU32,
    original_sample_rate: NonZeroU32,
    cache: Option<&dyn Cache>,
    decoder: &mut dyn AudioDecoder,
) -> Result<DecodedAudioF32, LoadError> {
    let mut pcm = None;

    let key = ResamplerKey {
        source_sample_rate: original_sample_rate,
        target_sample_rate,
        channels: probed.num_channels.get() as u16,
        quality: match config.resample_quality {
            ResampleQuality::VeryLow => 0,
            ResampleQuality::Low => 1,
            ResampleQuality::High => 2,
            ResampleQuality::HighWithLowLatency => 3,
        },
    };

    if let Some(cache) = cache
        && config.cache_resampler
    {
        cache.with_resampler_mut(key, probed, &mut |mut probed, resampler| {
            if resampler.nbr_channels() != probed.num_channels.get() {
                pcm = Some(Err(LoadError::InvalidResampler {
                    needed_channels: probed.num_channels.get(),
                    got_channels: resampler.nbr_channels(),
                }));
                return;
            }

            pcm = Some(decode::decode_resampled(
                probed.format_reader.as_mut(),
                config,
                target_sample_rate,
                original_sample_rate,
                probed.num_channels,
                resampler,
                decoder,
            ));

            resampler.reset();
        });
    } else {
        let mut resampler = key.create_resampler();

        pcm = Some(decode::decode_resampled(
            probed.format_reader.as_mut(),
            config,
            target_sample_rate,
            original_sample_rate,
            probed.num_channels,
            &mut resampler,
            decoder,
        ));
    }

    pcm.unwrap()
}

/// An audio source which has had its metadata probed, but has not been decoded yet.
pub struct ProbedAudioSource {
    format_reader: Box<dyn FormatReader>,
    sample_rate: Option<NonZeroU32>,
    num_channels: NonZeroUsize,
}

impl ProbedAudioSource {
    pub fn new(
        format_reader: Box<dyn FormatReader>,
        sample_rate: Option<NonZeroU32>,
        num_channels: NonZeroUsize,
    ) -> Self {
        Self {
            format_reader,
            sample_rate,
            num_channels,
        }
    }

    pub fn format_reader(&self) -> &dyn FormatReader {
        self.format_reader.as_ref()
    }

    /// The sample rate of the audio source.
    ///
    /// Returns `None` if the sample rate is unkown.
    pub fn sample_rate(&self) -> Option<NonZeroU32> {
        self.sample_rate
    }

    /// Override the sample rate of this resource with the given sample rate. This
    /// can be useful if [`ProbedAudioSource::sample_rate`] returns `None`, but you
    /// know what the sample rate is ahead of time.
    pub fn override_sample_rate(&mut self, sample_rate: NonZeroU32) {
        self.sample_rate = Some(sample_rate);
    }

    pub fn num_channels(&self) -> NonZeroUsize {
        self.num_channels
    }
}

impl From<ProbedAudioSource> for Box<dyn FormatReader> {
    fn from(value: ProbedAudioSource) -> Self {
        value.format_reader
    }
}

/// Additional settings for decoding audio data.
#[derive(Clone, Copy)]
pub struct DecodeConfig {
    /// The maximum size in bytes that the resulting `DecodedAudio`
    /// resource can be in RAM. If the resulting resource is larger than
    /// this, then an error will be returned instead. This is useful to
    /// avoid locking up or crashing the system if the user tries to load
    /// a really large audio file.
    ///
    /// By default, this is set to `1_000_000_000` (1GB).
    pub max_bytes: usize,

    /// The number of frames to initially allocate if the length of the
    /// audio source could not be determined up-front.
    ///
    /// By default, this is set to `32768`.
    pub default_alloc_frames: usize,

    /// Whether the decoded audio should be verified if possible during the
    /// decode process.
    ///
    /// By default, this is set to `false`.
    pub verify: bool,

    /// When enabled, the decoder will trim any delay and padding.
    ///
    /// By default, this is set to `true`.
    pub gapless: bool,

    /// If a [`Cache`] is present, then the decoder will be cached.
    ///
    /// By default, this is set to `true`.
    pub cache_decoder: bool,

    /// If a [`Cache`] is present, then the resampler will be cached if it
    /// is needed.
    ///
    /// This has no effect if the `resampler` feature is disabled, or when
    /// using `decode_stretched`.
    ///
    /// By default, this is set to `true`.
    pub cache_resampler: bool,

    /// Decode the track at the given index (the index into
    /// [`FormatReader::tracks()`]).
    ///
    /// Set to `None` to use the default audio track.
    ///
    /// By default, this is set to `None`.
    pub track_index: Option<usize>,

    /// The quality of the resampler to use if the `target_sample_rate`
    /// doesn't match the source sample rate.
    ///
    /// This has no effect if the source sample rate matches the target
    /// sample rate, or when using `decode_stretched`.
    ///
    /// By default, this is set to `ResampleQuality::default()` (High).
    #[cfg(feature = "resampler")]
    pub resample_quality: ResampleQuality,
}

impl Default for DecodeConfig {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_MAX_BYTES,
            default_alloc_frames: 32768,
            verify: false,
            gapless: true,
            cache_decoder: true,
            cache_resampler: true,
            track_index: None,
            #[cfg(feature = "resampler")]
            resample_quality: ResampleQuality::default(),
        }
    }
}

/// Additional settings for stretching audio data.
#[cfg(feature = "stretch-sinc-resampler")]
#[derive(Default, Clone, Copy)]
pub struct DecodeStretchedConfig {
    pub config: DecodeConfig,
    pub resampler_config: rubato::SincInterpolationParameters,
}

fn get_track(
    format_reader: &dyn FormatReader,
    track_index: Option<usize>,
) -> Result<&Track, LoadError> {
    if let Some(i) = track_index {
        let tracks = format_reader.tracks();
        let track = tracks.get(i).ok_or(LoadError::TrackIndexOutOfBounds {
            index: i,
            num_tracks: tracks.len(),
        })?;

        if track
            .codec_params
            .as_ref()
            .and_then(|p| p.audio())
            .is_none()
        {
            return Err(LoadError::NoAudioCodecFound);
        }

        Ok(track)
    } else {
        format_reader
            .default_track(TrackType::Audio)
            .ok_or(LoadError::NoAudioTrackFound)
    }
}

fn create_decoder(
    probed: &ProbedAudioSource,
    track_index: Option<usize>,
    options: &AudioDecoderOptions,
    codec_registry: &CodecRegistry,
) -> Result<Box<dyn AudioDecoder>, LoadError> {
    let track = get_track(probed.format_reader(), track_index)?;

    let codec_params = track.codec_params.as_ref().and_then(|p| p.audio()).unwrap();

    codec_registry
        .make_audio_decoder(codec_params, options)
        .map_err(LoadError::CouldNotCreateDecoder)
}
