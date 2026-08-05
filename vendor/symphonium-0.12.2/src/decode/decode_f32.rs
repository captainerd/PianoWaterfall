use std::num::{NonZeroU32, NonZeroUsize};

use symphonia::core::{
    audio::{
        Audio, AudioBuffer, GenericAudioBufferRef,
        conv::FromSample,
        sample::{i24, u24},
    },
    codecs::audio::AudioDecoder,
    formats::FormatReader,
};

use super::{alloc_final_buf, constrain_file_frames, decode_warning, shrink_buffer};
use crate::{DecodeConfig, DecodedAudioF32, error::LoadError, get_track};

pub(crate) fn decode_f32(
    format_reader: &mut dyn FormatReader,
    config: &DecodeConfig,
    num_channels: NonZeroUsize,
    sample_rate: NonZeroU32,
    original_sample_rate: NonZeroU32,
    decoder: &mut dyn AudioDecoder,
) -> Result<DecodedAudioF32, LoadError> {
    const BYTES_PER_SAMPLE: usize = 4;

    let track = get_track(format_reader, config.track_index)?;
    let track_id = track.id;

    let file_frames = constrain_file_frames(track.num_frames)?;

    let max_frames = config
        .max_bytes
        .checked_div(BYTES_PER_SAMPLE.checked_mul(num_channels.get()).unwrap())
        .unwrap();
    if let Some(frames) = file_frames
        && frames > max_frames
    {
        return Err(LoadError::FileTooLarge(config.max_bytes));
    }

    let mut final_buf: Vec<Vec<f32>> = alloc_final_buf(file_frames, num_channels, config);

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
                // Protect against really large files causing out of memory errors.
                if final_buf[0].len().saturating_add(decoded.frames()) > max_frames {
                    return Err(LoadError::FileTooLarge(config.max_bytes));
                }

                match decoded {
                    GenericAudioBufferRef::U8(p) => extend_f32_from_u8_packet(&mut final_buf, p),
                    GenericAudioBufferRef::S8(p) => extend_f32_from_i8_packet(&mut final_buf, p),
                    GenericAudioBufferRef::U16(p) => extend_f32_from_u16_packet(&mut final_buf, p),
                    GenericAudioBufferRef::S16(p) => extend_f32_from_i16_packet(&mut final_buf, p),
                    GenericAudioBufferRef::U24(p) => extend_f32_from_u24_packet(&mut final_buf, p),
                    GenericAudioBufferRef::S24(p) => extend_f32_from_i24_packet(&mut final_buf, p),
                    GenericAudioBufferRef::U32(p) => extend_f32_from_u32_packet(&mut final_buf, p),
                    GenericAudioBufferRef::S32(p) => extend_f32_from_i32_packet(&mut final_buf, p),
                    GenericAudioBufferRef::F32(p) => extend_f32_from_f32_packet(&mut final_buf, p),
                    GenericAudioBufferRef::F64(p) => extend_f32_from_f64_packet(&mut final_buf, p),
                }
            }
            Err(symphonia::core::errors::Error::DecodeError(err)) => decode_warning(err),
            Err(symphonia::core::errors::Error::IoError(err)) => decode_warning(err),
            Err(e) => return Err(LoadError::ErrorWhileDecoding(e)),
        }
    }

    shrink_buffer(&mut final_buf);

    Ok(DecodedAudioF32::new(
        final_buf,
        sample_rate,
        original_sample_rate,
    ))
}

macro_rules! extend_f32_from_packet {
    ($fn_name:ident, $packet_type:ty) => {
        fn $fn_name(final_buf: &mut [Vec<f32>], packet: &AudioBuffer<$packet_type>) {
            for (out_ch, in_ch) in final_buf.iter_mut().zip(packet.iter_planes()) {
                out_ch.reserve(in_ch.len());
                for in_s in in_ch.iter() {
                    out_ch.push(FromSample::from_sample(*in_s));
                }
            }
        }
    };
}

extend_f32_from_packet!(extend_f32_from_u8_packet, u8);
extend_f32_from_packet!(extend_f32_from_i8_packet, i8);
extend_f32_from_packet!(extend_f32_from_u16_packet, u16);
extend_f32_from_packet!(extend_f32_from_i16_packet, i16);
extend_f32_from_packet!(extend_f32_from_u24_packet, u24);
extend_f32_from_packet!(extend_f32_from_i24_packet, i24);
extend_f32_from_packet!(extend_f32_from_u32_packet, u32);
extend_f32_from_packet!(extend_f32_from_i32_packet, i32);
extend_f32_from_packet!(extend_f32_from_f64_packet, f64);

fn extend_f32_from_f32_packet(final_buf: &mut [Vec<f32>], packet: &AudioBuffer<f32>) {
    for (out_ch, in_ch) in final_buf.iter_mut().zip(packet.iter_planes()) {
        out_ch.extend_from_slice(in_ch);
    }
}
