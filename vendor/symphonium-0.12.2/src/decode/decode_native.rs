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

use super::{
    alloc_final_buf, check_total_frames, constrain_file_frames, decode_warning, shrink_buffer,
};
use crate::{DecodeConfig, DecodedAudio, DecodedAudioType, error::LoadError, get_track};

pub(crate) fn decode_native_bitdepth(
    format_reader: &mut dyn FormatReader,
    config: &DecodeConfig,
    num_channels: NonZeroUsize,
    sample_rate: NonZeroU32,
    original_sample_rate: NonZeroU32,
    decoder: &mut dyn AudioDecoder,
) -> Result<DecodedAudio, LoadError> {
    // Decode the first packet to get the sample format.
    let FirstPacketInfo {
        mut first_packet_type,
        max_frames,
        mut total_frames,
        track_id,
    } = decode_first_packet(
        format_reader,
        config,
        num_channels,
        config.max_bytes,
        decoder,
    )?;

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
                total_frames = total_frames.saturating_add(decoded.frames());
                if total_frames > max_frames {
                    return Err(LoadError::FileTooLarge(config.max_bytes));
                }

                match (decoded, &mut first_packet_type) {
                    (GenericAudioBufferRef::U8(decoded), FirstPacketType::U8(final_buf)) => {
                        extend_from_u8_packet(final_buf, decoded);
                    }
                    (GenericAudioBufferRef::S8(decoded), FirstPacketType::S8(final_buf)) => {
                        extend_from_i8_packet(final_buf, decoded);
                    }
                    (GenericAudioBufferRef::U16(decoded), FirstPacketType::U16(final_buf)) => {
                        extend_from_u16_packet(final_buf, decoded);
                    }
                    (GenericAudioBufferRef::S16(decoded), FirstPacketType::S16(final_buf)) => {
                        extend_from_i16_packet(final_buf, decoded);
                    }
                    (GenericAudioBufferRef::U24(decoded), FirstPacketType::U24(final_buf)) => {
                        extend_from_u24_packet(final_buf, decoded);
                    }
                    (GenericAudioBufferRef::S24(decoded), FirstPacketType::S24(final_buf)) => {
                        extend_from_i24_packet(final_buf, decoded);
                    }
                    (GenericAudioBufferRef::U32(decoded), FirstPacketType::U32(final_buf)) => {
                        extend_from_u32_packet(final_buf, decoded);
                    }
                    (GenericAudioBufferRef::S32(decoded), FirstPacketType::S32(final_buf)) => {
                        extend_from_i32_packet(final_buf, decoded);
                    }
                    (GenericAudioBufferRef::F32(decoded), FirstPacketType::F32(final_buf)) => {
                        extend_from_f32_packet(final_buf, decoded);
                    }
                    (GenericAudioBufferRef::F64(decoded), FirstPacketType::F64(final_buf)) => {
                        extend_from_f64_packet(final_buf, decoded);
                    }
                    _ => {
                        return Err(LoadError::ErrorWhileDecoding(
                            symphonia::core::errors::Error::DecodeError("Invalid packet type"),
                        ));
                    }
                }
            }
            Err(symphonia::core::errors::Error::DecodeError(err)) => decode_warning(err),
            Err(symphonia::core::errors::Error::IoError(err)) => decode_warning(err),
            Err(e) => return Err(LoadError::ErrorWhileDecoding(e)),
        }
    }

    let resource_type = match first_packet_type {
        FirstPacketType::U8(mut b) => {
            shrink_buffer(&mut b);
            DecodedAudioType::U8(b)
        }
        FirstPacketType::S8(mut b) => {
            shrink_buffer(&mut b);
            DecodedAudioType::S8(b)
        }
        FirstPacketType::U16(mut b) => {
            shrink_buffer(&mut b);
            DecodedAudioType::U16(b)
        }
        FirstPacketType::S16(mut b) => {
            shrink_buffer(&mut b);
            DecodedAudioType::S16(b)
        }
        FirstPacketType::U24(mut b) => {
            shrink_buffer(&mut b);
            DecodedAudioType::U24(b)
        }
        FirstPacketType::S24(mut b) => {
            shrink_buffer(&mut b);
            DecodedAudioType::U24(b)
        }
        FirstPacketType::U32(mut b) => {
            shrink_buffer(&mut b);
            DecodedAudioType::U32(b)
        }
        FirstPacketType::S32(mut b) => {
            shrink_buffer(&mut b);
            DecodedAudioType::S32(b)
        }
        FirstPacketType::F32(mut b) => {
            shrink_buffer(&mut b);
            DecodedAudioType::F32(b)
        }
        FirstPacketType::F64(mut b) => {
            shrink_buffer(&mut b);
            DecodedAudioType::F64(b)
        }
    };

    Ok(DecodedAudio::new(
        resource_type,
        sample_rate,
        original_sample_rate,
    ))
}

struct FirstPacketInfo {
    first_packet_type: FirstPacketType,
    max_frames: usize,
    total_frames: usize,
    track_id: u32,
}

enum FirstPacketType {
    U8(Vec<Vec<u8>>),
    S8(Vec<Vec<i8>>),
    U16(Vec<Vec<u16>>),
    S16(Vec<Vec<i16>>),
    U24(Vec<Vec<[u8; 3]>>),
    S24(Vec<Vec<[u8; 3]>>),
    U32(Vec<Vec<u32>>),
    S32(Vec<Vec<i32>>),
    F32(Vec<Vec<f32>>),
    F64(Vec<Vec<f64>>),
}

fn decode_first_packet(
    format_reader: &mut dyn FormatReader,
    config: &DecodeConfig,
    num_channels: NonZeroUsize,
    max_bytes: usize,
    decoder: &mut dyn AudioDecoder,
) -> Result<FirstPacketInfo, LoadError> {
    let track = get_track(format_reader, config.track_index)?;
    let track_id = track.id;

    let file_frames = constrain_file_frames(track.num_frames)?;

    let mut max_frames = 0;
    let mut total_frames = 0;
    let mut first_packet = None;

    macro_rules! process_packet {
        ($packet:ident, $extend_fn:ident, $buf_type:ty, $variant:ident) => {{
            max_frames = max_bytes
                .checked_div(
                    std::mem::size_of::<$buf_type>()
                        .checked_mul(num_channels.get())
                        .unwrap(),
                )
                .unwrap();
            if let Some(file_frames) = file_frames
                && file_frames > max_frames
            {
                return Err(LoadError::FileTooLarge(max_bytes));
            } else {
                check_total_frames(&mut total_frames, max_frames, $packet.frames(), max_bytes)?;
            }

            let mut final_buf = alloc_final_buf::<$buf_type>(file_frames, num_channels, config);

            ($extend_fn)(&mut final_buf, $packet);

            first_packet = Some(FirstPacketType::$variant(final_buf));
            break;
        }};
    }

    while let Some(packet) = format_reader
        .next_packet()
        .map_err(LoadError::ErrorWhileDecoding)?
    {
        // If the packet does not belong to the selected track, skip over it.
        if packet.track_id != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => match decoded {
                GenericAudioBufferRef::U8(p) => {
                    process_packet!(p, extend_from_u8_packet, u8, U8)
                }
                GenericAudioBufferRef::S8(p) => process_packet!(p, extend_from_i8_packet, i8, S8),
                GenericAudioBufferRef::U16(p) => {
                    process_packet!(p, extend_from_u16_packet, u16, U16)
                }
                GenericAudioBufferRef::S16(p) => {
                    process_packet!(p, extend_from_i16_packet, i16, S16)
                }
                GenericAudioBufferRef::U24(p) => {
                    process_packet!(p, extend_from_u24_packet, [u8; 3], U24)
                }
                GenericAudioBufferRef::S24(p) => {
                    process_packet!(p, extend_from_i24_packet, [u8; 3], S24)
                }
                GenericAudioBufferRef::U32(p) => {
                    process_packet!(p, extend_from_u32_packet, u32, U32)
                }
                GenericAudioBufferRef::S32(p) => {
                    process_packet!(p, extend_from_i32_packet, i32, S32)
                }
                GenericAudioBufferRef::F32(p) => {
                    process_packet!(p, extend_from_f32_packet, f32, F32)
                }
                GenericAudioBufferRef::F64(p) => {
                    process_packet!(p, extend_from_f64_packet, f64, F64)
                }
            },
            Err(symphonia::core::errors::Error::DecodeError(err)) => decode_warning(err),
            Err(symphonia::core::errors::Error::IoError(err)) => decode_warning(err),
            Err(e) => return Err(LoadError::ErrorWhileDecoding(e)),
        };
    }

    first_packet
        .map(|packet_type| FirstPacketInfo {
            first_packet_type: packet_type,
            max_frames,
            total_frames,
            track_id,
        })
        .ok_or(LoadError::ErrorWhileDecoding(
            symphonia::core::errors::Error::DecodeError("No valid packets found"),
        ))
}

macro_rules! extend_from_packet {
    ($fn_name:ident, $packet_type:ty) => {
        fn $fn_name(final_buf: &mut [Vec<$packet_type>], packet: &AudioBuffer<$packet_type>) {
            for (out_ch, in_ch) in final_buf.iter_mut().zip(packet.iter_planes()) {
                out_ch.extend_from_slice(in_ch);
            }
        }
    };
}

extend_from_packet!(extend_from_u8_packet, u8);
extend_from_packet!(extend_from_i8_packet, i8);
extend_from_packet!(extend_from_u16_packet, u16);
extend_from_packet!(extend_from_i16_packet, i16);
extend_from_packet!(extend_from_u32_packet, u32);
extend_from_packet!(extend_from_i32_packet, i32);
extend_from_packet!(extend_from_f32_packet, f32);
extend_from_packet!(extend_from_f64_packet, f64);

fn extend_from_u24_packet(final_buf: &mut [Vec<[u8; 3]>], packet: &AudioBuffer<u24>) {
    for (out_ch, in_ch) in final_buf.iter_mut().zip(packet.iter_planes()) {
        out_ch.reserve(in_ch.len());
        for in_s in in_ch.iter() {
            out_ch.push(in_s.to_ne_bytes());
        }
    }
}

fn extend_from_i24_packet(final_buf: &mut [Vec<[u8; 3]>], packet: &AudioBuffer<i24>) {
    for (out_ch, in_ch) in final_buf.iter_mut().zip(packet.iter_planes()) {
        out_ch.reserve(in_ch.len());
        for in_s in in_ch.iter() {
            // Note, symphonium's implementation of `i24::to_ne_bytes()` is incorrect.
            // Just convert the sample to u24 format instead.
            let s_u24: u24 = FromSample::from_sample(*in_s);
            out_ch.push(s_u24.to_ne_bytes());
        }
    }
}
