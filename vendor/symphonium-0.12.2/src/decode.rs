use std::{convert::TryFrom, fmt::Display, num::NonZeroUsize};

#[cfg(all(feature = "log", not(feature = "tracing")))]
use log::warn;
#[cfg(feature = "tracing")]
use tracing::warn;

use crate::{DecodeConfig, error::LoadError};

const SHRINK_THRESHOLD: usize = 4096;

mod decode_f32;
pub(crate) use decode_f32::*;

#[cfg(feature = "decode-native")]
mod decode_native;
#[cfg(feature = "decode-native")]
pub(crate) use decode_native::*;

#[cfg(feature = "resampler")]
mod decode_resampled_f32;
#[cfg(feature = "resampler")]
pub(crate) use decode_resampled_f32::*;

#[cfg(feature = "decode-native")]
fn check_total_frames(
    total_frames: &mut usize,
    max_frames: usize,
    packet_len: usize,
    max_bytes: usize,
) -> Result<(), super::LoadError> {
    *total_frames = total_frames.saturating_add(packet_len);
    if *total_frames > max_frames {
        Err(super::LoadError::FileTooLarge(max_bytes))
    } else {
        Ok(())
    }
}

fn shrink_buffer<T>(channels: &mut [Vec<T>]) {
    for ch in channels.iter_mut() {
        // If the allocated capacity is significantly greater than the
        // length, shrink it to save memory.
        if ch.capacity() > ch.len().saturating_add(SHRINK_THRESHOLD) {
            ch.shrink_to_fit();
        }
    }
}

fn decode_warning(err: impl Display) {
    #[cfg(any(feature = "tracing", feature = "log"))]
    // Decode errors are not fatal. Print the error message and try to decode the next
    // packet as usual.
    warn!("Symphonia decode warning: {}", err);

    #[cfg(not(any(feature = "tracing", feature = "log")))]
    let _ = err;
}

fn alloc_final_buf<T>(
    file_frames: Option<usize>,
    num_channels: NonZeroUsize,
    config: &DecodeConfig,
) -> Vec<Vec<T>> {
    let mut final_buf = Vec::new();
    final_buf.reserve_exact(num_channels.get());

    for _ in 0..num_channels.get() {
        let mut v = Vec::new();

        if let Some(file_frames) = file_frames {
            v.reserve_exact(file_frames);
        } else {
            v.reserve(config.default_alloc_frames);
        }

        final_buf.push(v);
    }

    final_buf
}

fn constrain_file_frames(file_frames: Option<u64>) -> Result<Option<usize>, LoadError> {
    if let Some(frames) = file_frames {
        Ok(Some(
            usize::try_from(frames).map_err(|_| LoadError::FileTooLarge(usize::MAX))?,
        ))
    } else {
        Ok(None)
    }
}
