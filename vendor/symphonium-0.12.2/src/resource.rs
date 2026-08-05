use std::num::NonZeroU32;

#[cfg(feature = "decode-native")]
use symphonia::core::audio::{conv::FromSample, sample::u24};

/// A resource of raw f32 audio samples stored in deinterleaved format.
#[derive(Clone)]
pub struct DecodedAudioF32 {
    pub data: Vec<Vec<f32>>,
    /// The sample rate of this audio resource.
    pub sample_rate: NonZeroU32,
    /// The sample rate of the audio resource before it was resampled
    /// (if it was resampled).
    pub original_sample_rate: NonZeroU32,
}

impl DecodedAudioF32 {
    /// Construct a new `DecedAudioF32` resource.
    ///
    /// * `data` - The deinterleaved audio data.
    /// * `sample_rate` - The sample rate of this resource.
    /// * `original_sample_rate` - The sample rate of the audio resource before
    ///   it was resampled (if it was resampled).
    pub fn new(
        data: Vec<Vec<f32>>,
        sample_rate: NonZeroU32,
        original_sample_rate: NonZeroU32,
    ) -> Self {
        let frames = data[0].len();

        for ch in data.iter().skip(1) {
            assert_eq!(ch.len(), frames);
        }

        Self {
            data,
            sample_rate,
            original_sample_rate,
        }
    }

    /// The number of channels in this resource.
    pub fn channels(&self) -> usize {
        self.data.len()
    }

    /// The length of this resource in frames (length of a single channel in
    /// samples).
    pub fn frames(&self) -> usize {
        self.data[0].len()
    }
}

impl std::fmt::Debug for DecodedAudioF32 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "DecodedAudioF32 {{ channels: {}, frames: {}, sample_rate: {} }}",
            self.data.len(),
            self.data[0].len(),
            self.sample_rate
        )
    }
}

#[cfg(feature = "decode-native")]
impl From<DecodedAudioF32> for DecodedAudio {
    fn from(value: DecodedAudioF32) -> Self {
        let channels = value.channels();
        let frames = value.frames();

        DecodedAudio {
            resource_type: DecodedAudioType::F32(value.data),
            sample_rate: value.sample_rate,
            original_sample_rate: value.original_sample_rate,
            channels,
            frames,
        }
    }
}

/// A resource of raw audio samples stored in deinterleaved format.
///
/// This struct stores samples
/// in their native sample format when possible to save memory.
#[cfg(feature = "decode-native")]
#[derive(Debug, Clone)]
pub struct DecodedAudio {
    resource_type: DecodedAudioType,
    sample_rate: NonZeroU32,
    original_sample_rate: NonZeroU32,
    channels: usize,
    frames: usize,
}

/// The format of the raw audio samples stored in deinterleaved format.
///
/// Note there is no option for `I24` (signed 24 bit). This is because
/// converting two's compliment numbers from 32 bit to 24 bit and vice-versa
/// is tricky and less performant. Instead, just convert the data into `u24`
/// format.
#[cfg(feature = "decode-native")]
#[derive(Clone)]
pub enum DecodedAudioType {
    U8(Vec<Vec<u8>>),
    S8(Vec<Vec<i8>>),
    U16(Vec<Vec<u16>>),
    S16(Vec<Vec<i16>>),
    /// The data is stored in native-endian byte order.
    U24(Vec<Vec<[u8; 3]>>),
    U32(Vec<Vec<u32>>),
    S32(Vec<Vec<i32>>),
    F32(Vec<Vec<f32>>),
    F64(Vec<Vec<f64>>),
}

#[cfg(feature = "decode-native")]
impl DecodedAudio {
    /// Construct a new `DecodedAudio` resource.
    ///
    /// * `resource_type` - The deinterleaved audio data.
    /// * `sample_rate` - The sample rate of this resource.
    /// * `original_sample_rate` - The sample rate of the audio resource before
    ///   it was resampled (if it was resampled).
    pub fn new(
        resource_type: DecodedAudioType,
        sample_rate: NonZeroU32,
        original_sample_rate: NonZeroU32,
    ) -> Self {
        let (channels, frames) = match &resource_type {
            DecodedAudioType::U8(b) => {
                let len = b[0].len();

                for ch in b.iter().skip(1) {
                    assert_eq!(ch.len(), len);
                }

                (b.len(), len)
            }
            DecodedAudioType::S8(b) => {
                let len = b[0].len();

                for ch in b.iter().skip(1) {
                    assert_eq!(ch.len(), len);
                }

                (b.len(), len)
            }
            DecodedAudioType::U16(b) => {
                let len = b[0].len();

                for ch in b.iter().skip(1) {
                    assert_eq!(ch.len(), len);
                }

                (b.len(), len)
            }
            DecodedAudioType::S16(b) => {
                let len = b[0].len();

                for ch in b.iter().skip(1) {
                    assert_eq!(ch.len(), len);
                }

                (b.len(), len)
            }
            DecodedAudioType::U24(b) => {
                let len = b[0].len();

                for ch in b.iter().skip(1) {
                    assert_eq!(ch.len(), len);
                }

                (b.len(), len)
            }
            DecodedAudioType::U32(b) => {
                let len = b[0].len();

                for ch in b.iter().skip(1) {
                    assert_eq!(ch.len(), len);
                }

                (b.len(), len)
            }
            DecodedAudioType::S32(b) => {
                let len = b[0].len();

                for ch in b.iter().skip(1) {
                    assert_eq!(ch.len(), len);
                }

                (b.len(), len)
            }
            DecodedAudioType::F32(b) => {
                let len = b[0].len();

                for ch in b.iter().skip(1) {
                    assert_eq!(ch.len(), len);
                }

                (b.len(), len)
            }
            DecodedAudioType::F64(b) => {
                let len = b[0].len();

                for ch in b.iter().skip(1) {
                    assert_eq!(ch.len(), len);
                }

                (b.len(), len)
            }
        };

        Self {
            resource_type,
            sample_rate,
            original_sample_rate,
            channels,
            frames,
        }
    }

    /// The number of channels in this resource.
    pub fn channels(&self) -> usize {
        self.channels
    }

    /// The length of this resource in frames (length of a single channel in
    /// samples).
    pub fn frames(&self) -> usize {
        self.frames
    }

    /// The sample rate of this resource in samples per second.
    pub fn sample_rate(&self) -> NonZeroU32 {
        self.sample_rate
    }

    /// The sample rate of the audio resource before it was resampled
    /// (if it was resampled).
    pub fn original_sample_rate(&self) -> NonZeroU32 {
        self.original_sample_rate
    }

    /// Get an immutable reference to the raw sample data.
    pub fn raw(&self) -> &DecodedAudioType {
        &self.resource_type
    }

    /// Get a mutable reference to the raw sample data.
    ///
    /// Please do not change the length of any of the returned vecs.
    pub fn raw_mut(&mut self) -> &mut DecodedAudioType {
        &mut self.resource_type
    }

    /// Fill the buffer with samples from the given `channel`, starting from the
    /// given `frame`.
    ///
    /// If the length of the buffer exceeds the length of the PCM resource, then
    /// the remaining samples will be filled with zeros.
    ///
    /// This returns the number of frames that were copied into the buffer. (If
    /// this number is less than the length of `buf`, then it means that the
    /// remaining samples were filled with zeros.)
    ///
    /// The will return an error if the given channel does not exist.
    pub fn fill_channel(
        &self,
        channel: usize,
        frame: usize,
        buf: &mut [f32],
    ) -> Result<usize, FillChannelError> {
        if channel >= self.channels {
            return Err(FillChannelError {
                got_index: channel,
                num_channels: self.channels,
            });
        }

        if frame >= self.frames {
            // Out of range, fill with zeros instead.
            buf.fill(0.0);
            return Ok(0);
        }

        let fill_frames = if frame.saturating_add(buf.len()) > self.frames {
            // Fill the out-of-range part with zeros.
            let fill_frames = self.frames.saturating_sub(frame);
            buf[fill_frames..].fill(0.0);
            fill_frames
        } else {
            buf.len()
        };
        let end_frame = frame.saturating_add(fill_frames);

        let buf_part = &mut buf[0..fill_frames];

        match &self.resource_type {
            DecodedAudioType::U8(pcm) => {
                let pcm_part = &pcm[channel][frame..end_frame];

                for i in 0..fill_frames {
                    buf_part[i] = FromSample::from_sample(pcm_part[i]);
                }
            }
            DecodedAudioType::S8(pcm) => {
                let pcm_part = &pcm[channel][frame..end_frame];

                for i in 0..fill_frames {
                    buf_part[i] = FromSample::from_sample(pcm_part[i]);
                }
            }
            DecodedAudioType::U16(pcm) => {
                let pcm_part = &pcm[channel][frame..end_frame];

                for i in 0..fill_frames {
                    buf_part[i] = FromSample::from_sample(pcm_part[i]);
                }
            }
            DecodedAudioType::S16(pcm) => {
                let pcm_part = &pcm[channel][frame..end_frame];

                for i in 0..fill_frames {
                    buf_part[i] = FromSample::from_sample(pcm_part[i]);
                }
            }
            DecodedAudioType::U24(pcm) => {
                let pcm_part = &pcm[channel][frame..end_frame];

                for i in 0..fill_frames {
                    let b = &pcm_part[i];

                    #[cfg(target_endian = "little")]
                    let s = u24(u32::from_ne_bytes([b[0], b[1], b[2], 0]));

                    #[cfg(target_endian = "big")]
                    let s = u24(u32::from_ne_bytes([0, b[0], b[1], b[2]]));

                    buf_part[i] = FromSample::from_sample(s);
                }
            }
            DecodedAudioType::U32(pcm) => {
                let pcm_part = &pcm[channel][frame..end_frame];

                for i in 0..fill_frames {
                    buf_part[i] = FromSample::from_sample(pcm_part[i]);
                }
            }
            DecodedAudioType::S32(pcm) => {
                let pcm_part = &pcm[channel][frame..end_frame];

                for i in 0..fill_frames {
                    buf_part[i] = FromSample::from_sample(pcm_part[i]);
                }
            }
            DecodedAudioType::F32(pcm) => {
                let pcm_part = &pcm[channel][frame..end_frame];

                buf_part.copy_from_slice(pcm_part);
            }
            DecodedAudioType::F64(pcm) => {
                let pcm_part = &pcm[channel][frame..end_frame];

                for i in 0..fill_frames {
                    buf_part[i] = pcm_part[i] as f32;
                }
            }
        }

        Ok(fill_frames)
    }

    /// Fill the stereo buffer with samples, starting from the given `frame`.
    ///
    /// If this resource has only one channel, then both channels will be
    /// filled with the same data.
    ///
    /// If the length of the buffer exceeds the length of the PCM resource, then
    /// the remaining samples will be filled with zeros.
    ///
    /// This returns the number of frames that were copied into the buffer. (If
    /// this number is less than the length of `buf`, then it means that the
    /// remaining samples were filled with zeros.)
    pub fn fill_stereo(&self, frame: usize, buf_l: &mut [f32], buf_r: &mut [f32]) -> usize {
        let buf_len = buf_l.len().min(buf_r.len());
        let buf_l = &mut buf_l[..buf_len];
        let buf_r = &mut buf_r[..buf_len];

        let fill_frames = if self.channels > 0 {
            self.fill_channel(0, frame, buf_l).unwrap()
        } else {
            return 0;
        };

        if self.channels > 1 {
            self.fill_channel(1, frame, buf_r).unwrap();
        } else {
            buf_r.copy_from_slice(buf_l);
        }

        fill_frames
    }

    /// Consume this resource and return the raw samples.
    pub fn into_raw(self) -> DecodedAudioType {
        self.resource_type
    }
}

#[cfg(feature = "decode-native")]
impl std::fmt::Debug for DecodedAudioType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::U8(c) => write!(
                f,
                "DecodedAudioType::U8 {{ channels: {}, frames: {} }}",
                c.len(),
                c[0].len()
            ),
            Self::S8(c) => write!(
                f,
                "DecodedAudioType::S8 {{ channels: {}, frames: {} }}",
                c.len(),
                c[0].len()
            ),
            Self::U16(c) => write!(
                f,
                "DecodedAudioType::U16 {{ channels: {}, frames: {} }}",
                c.len(),
                c[0].len()
            ),
            Self::S16(c) => write!(
                f,
                "DecodedAudioType::S16 {{ channels: {}, frames: {} }}",
                c.len(),
                c[0].len()
            ),
            Self::U24(c) => write!(
                f,
                "DecodedAudioType::U24 {{ channels: {}, frames: {} }}",
                c.len(),
                c[0].len()
            ),
            Self::U32(c) => write!(
                f,
                "DecodedAudioType::U32 {{ channels: {}, frames: {} }}",
                c.len(),
                c[0].len()
            ),
            Self::S32(c) => write!(
                f,
                "DecodedAudioType::S32 {{ channels: {}, frames: {} }}",
                c.len(),
                c[0].len()
            ),
            Self::F32(c) => write!(
                f,
                "DecodedAudioType::F32 {{ channels: {}, frames: {} }}",
                c.len(),
                c[0].len()
            ),
            Self::F64(c) => write!(
                f,
                "DecodedAudioType::F64 {{ channels: {}, frames: {} }}",
                c.len(),
                c[0].len()
            ),
        }
    }
}

#[cfg(all(test, feature = "decode-native"))]
mod tests {
    use super::*;

    #[test]
    fn pcm_fill_range_test() {
        let test_pcm = DecodedAudio::new(
            DecodedAudioType::F32(vec![vec![1.0, 2.0, 3.0, 4.0]]),
            NonZeroU32::new(44100).unwrap(),
            NonZeroU32::new(44100).unwrap(),
        );

        let mut out_buf: [f32; 8] = [10.0; 8];

        let fill_frames = test_pcm.fill_channel(0, 0, &mut out_buf[0..4]);
        assert_eq!(fill_frames, Ok(4));
        assert_eq!(&out_buf[0..4], &[1.0, 2.0, 3.0, 4.0]);

        out_buf = [10.0; 8];
        let fill_frames = test_pcm.fill_channel(0, 0, &mut out_buf[0..5]);
        assert_eq!(fill_frames, Ok(4));
        assert_eq!(&out_buf[0..5], &[1.0, 2.0, 3.0, 4.0, 0.0]);

        out_buf = [10.0; 8];
        let fill_frames = test_pcm.fill_channel(0, 2, &mut out_buf[0..4]);
        assert_eq!(fill_frames, Ok(2));
        assert_eq!(&out_buf[0..4], &[3.0, 4.0, 0.0, 0.0]);

        out_buf = [10.0; 8];
        let fill_frames = test_pcm.fill_channel(0, 3, &mut out_buf[0..4]);
        assert_eq!(fill_frames, Ok(1));
        assert_eq!(&out_buf[0..4], &[4.0, 0.0, 0.0, 0.0]);

        out_buf = [10.0; 8];
        let fill_frames = test_pcm.fill_channel(0, 4, &mut out_buf[0..4]);
        assert_eq!(fill_frames, Ok(0));
        assert_eq!(&out_buf[0..4], &[0.0, 0.0, 0.0, 0.0]);

        out_buf = [10.0; 8];
        let fill_frames = test_pcm.fill_channel(0, 1, &mut out_buf[0..2]);
        assert_eq!(fill_frames, Ok(2));
        assert_eq!(&out_buf[0..2], &[2.0, 3.0]);

        out_buf = [10.0; 8];
        let fill_frames = test_pcm.fill_channel(0, 1, &mut out_buf[0..4]);
        assert_eq!(fill_frames, Ok(3));
        assert_eq!(&out_buf[0..4], &[2.0, 3.0, 4.0, 0.0]);
    }
}

/// An error occured while using [`DecodedAudio::fill_channel`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FillChannelError {
    pub got_index: usize,
    pub num_channels: usize,
}

impl std::error::Error for FillChannelError {}

impl std::fmt::Display for FillChannelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "channel index {} out of bounds for DecodedAudio with {} channels",
            self.got_index, self.num_channels
        )
    }
}
