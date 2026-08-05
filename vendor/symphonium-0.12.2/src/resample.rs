use std::num::NonZeroU32;

pub use fixed_resample;
pub use fixed_resample::ResampleQuality;
use fixed_resample::{PacketResampler, Sequential};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResamplerKey {
    pub source_sample_rate: NonZeroU32,
    pub target_sample_rate: NonZeroU32,
    pub channels: u16,
    pub quality: u16,
}

impl ResamplerKey {
    pub fn create_resampler(&self) -> PacketResampler<f32, Sequential<f32>> {
        PacketResampler::new(
            self.channels as usize,
            self.source_sample_rate.get(),
            self.target_sample_rate.get(),
            fixed_resample::ResamplerConfig {
                quality: match self.quality {
                    0 => ResampleQuality::VeryLow,
                    1 => ResampleQuality::Low,
                    2 => ResampleQuality::High,
                    _ => ResampleQuality::HighWithLowLatency,
                },
                ..Default::default()
            },
        )
    }
}
