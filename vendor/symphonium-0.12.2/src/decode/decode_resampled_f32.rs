use std::num::{NonZeroU32, NonZeroUsize};

use fixed_resample::rubato::audioadapter::Adapter;
use fixed_resample::{
    PacketResampler, Sequential, audioadapter_buffers::direct::SequentialSliceOfSlices,
};
use symphonia::core::audio::{Audio, AudioBuffer};
use symphonia::core::codecs::audio::AudioDecoder;
use symphonia::core::formats::FormatReader;

use super::{alloc_final_buf, constrain_file_frames, decode_warning, shrink_buffer};
use crate::{DecodeConfig, DecodedAudioF32, error::LoadError, get_track};

const MAX_TMP_BUFFER_FRAMES: usize = 1_000_000;

pub(crate) fn decode_resampled(
    format_reader: &mut dyn FormatReader,
    config: &DecodeConfig,
    sample_rate: NonZeroU32,
    original_sample_rate: NonZeroU32,
    num_channels: NonZeroUsize,
    resampler: &mut PacketResampler<f32, Sequential<f32>>,
    decoder: &mut dyn AudioDecoder,
) -> Result<DecodedAudioF32, LoadError> {
    use fixed_resample::LastPacketInfo;

    const BYTES_PER_SAMPLE: usize = 4;

    let track = get_track(format_reader, config.track_index)?;
    let track_id = track.id;

    let file_frames = if let Some(f) = track.num_frames {
        constrain_file_frames(Some(resampler.out_alloc_frames(f)))?
    } else {
        None
    };

    let max_frames = config
        .max_bytes
        .checked_div(BYTES_PER_SAMPLE.checked_mul(num_channels.get()).unwrap())
        .unwrap();
    if let Some(frames) = file_frames
        && frames > max_frames
    {
        return Err(LoadError::FileTooLarge(config.max_bytes));
    }

    let max_frames_per_packet = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .and_then(|p| p.max_frames_per_packet)
        .and_then(|f| {
            usize::try_from(f)
                .ok()
                .and_then(|f| (f < MAX_TMP_BUFFER_FRAMES).then_some(f))
        });

    let mut tmp_conversion_buf: Option<AudioBuffer<f32>> = None;
    let mut tmp_slice_buf: Vec<&[f32]> = Vec::new();
    let mut final_buf: Vec<Vec<f32>> = alloc_final_buf(file_frames, num_channels, config);

    let mut total_in_frames: u64 = 0;

    while let Some(packet) = format_reader
        .next_packet()
        .map_err(LoadError::ErrorWhileDecoding)?
    {
        // If the packet does not belong to the selected track, skip over it.
        if packet.track_id != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                let decoded_frames = decoded.frames();

                // If this is the first decoded packet, allocate the temporary conversion
                // buffer with the required capacity.
                let tmp_conversion_buf = tmp_conversion_buf.get_or_insert_with(|| {
                    let spec = decoded.spec().clone();
                    let capacity = max_frames_per_packet.unwrap_or(decoded.capacity());

                    AudioBuffer::new(spec, capacity)
                });
                if tmp_conversion_buf.capacity() < decoded_frames {
                    let spec = decoded.spec().clone();
                    let capacity = decoded.capacity();

                    *tmp_conversion_buf = AudioBuffer::new(spec, capacity);
                }

                tmp_conversion_buf.resize_uninit(decoded_frames);
                decoded.copy_to(tmp_conversion_buf);

                total_in_frames = total_in_frames.saturating_add(decoded_frames as u64);

                tmp_slice_buf = tmp_conversion_buf.iter_planes().collect();

                let mut file_too_large = false;

                resampler.process(
                    &SequentialSliceOfSlices::new(
                        &tmp_slice_buf,
                        num_channels.get(),
                        decoded_frames,
                    )
                    .unwrap(),
                    None,
                    None,
                    |in_buf, frames| {
                        // Protect against really large files causing out of memory errors.
                        if !file_too_large && final_buf[0].len().saturating_add(frames) > max_frames
                        {
                            file_too_large = true;
                        } else {
                            for ch in 0..in_buf.channels().min(final_buf.len()) {
                                fixed_resample::extend_from_adapter_channel(
                                    &mut final_buf[ch],
                                    in_buf,
                                    0,
                                    ch,
                                    frames,
                                );
                            }
                        }
                    },
                    None,
                    true, // trim_delay
                );

                if file_too_large {
                    return Err(LoadError::FileTooLarge(config.max_bytes));
                }
            }
            Err(symphonia::core::errors::Error::DecodeError(err)) => decode_warning(err),
            Err(symphonia::core::errors::Error::IoError(err)) => decode_warning(err),
            Err(e) => return Err(LoadError::ErrorWhileDecoding(e)),
        }
    }

    // Process any leftover samples in the resampler.
    let final_output_frames = usize::try_from(resampler.out_alloc_frames(total_in_frames))
        .map_err(|_| LoadError::FileTooLarge(config.max_bytes))?;

    if let Some(desired_output_frames) = final_output_frames.checked_sub(final_buf[0].len())
        && !tmp_slice_buf.is_empty()
    {
        resampler.process(
            &SequentialSliceOfSlices::new(&tmp_slice_buf, num_channels.get(), 0).unwrap(),
            Some(0..0),
            None,
            |in_buf, frames| {
                for ch in 0..in_buf.channels().min(final_buf.len()) {
                    fixed_resample::extend_from_adapter_channel(
                        &mut final_buf[ch],
                        in_buf,
                        0,
                        ch,
                        frames,
                    );
                }
            },
            Some(LastPacketInfo {
                desired_output_frames: Some(desired_output_frames as u64),
            }),
            true, // trim_delay
        );
    } else {
        for ch in final_buf.iter_mut() {
            ch.resize(final_output_frames, 0.0);
        }
    }

    shrink_buffer(&mut final_buf);

    Ok(DecodedAudioF32::new(
        final_buf,
        sample_rate,
        original_sample_rate,
    ))
}
