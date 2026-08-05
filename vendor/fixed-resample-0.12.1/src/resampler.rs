use audioadapter_buffers::direct;
use audioadapter_buffers::owned::{InterleavedOwned, SequentialOwned};
use rubato::{
    audioadapter::{Adapter, AdapterMut},
    ResampleResult, Resampler, Sample,
};
use std::ops::Range;

/// The quality of the resampling algorithm used for a [`PacketResampler`] or a
/// [`Resampler`] created with [`resampler_from_quality`].
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResampleQuality {
    /// Low quality, low CPU, low latency
    ///
    /// Internally this uses the [`Async`][rubato::Async] resampler from rubato
    /// with linear polynomial interpolation.
    VeryLow,
    /// Slightly better quality than [`ResampleQuality::VeryLow`], slightly higher
    /// CPU than [`ResampleQuality::VeryLow`], low latency
    ///
    /// Internally this uses the [`Async`][rubato::Async] resampler from rubato
    /// with cubic polynomial interpolation.
    Low,
    #[default]
    /// Great quality, medium CPU, high latency
    ///
    /// This is recommended for most non-realtime applications where higher
    /// latency is not an issue.
    ///
    /// Note, this resampler type adds a significant amount of latency (in
    /// the hundreds of frames), so prefer to use the "Low" option if low
    /// latency is desired.
    ///
    /// If the `fft-resampler` feature is not enabled, then this will fall
    /// back to [`ResampleQuality::Low`].
    ///
    /// Internally this uses the [`rubato::Fft`] resampler from rubato.
    High,
    /// Great quality, high CPU, low latency
    ///
    /// Internally this uses the [`Async`][rubato::Async] resampler from rubato
    /// with [`Cubic`](rubato::SincInterpolationType::Cubic) sinc
    /// interpolation, a [`BlackmanHarris2`](rubato::WindowFunction::BlackmanHarris2)
    /// window function, a sinc length of `256`, and an oversampling factor
    /// of `128`.
    HighWithLowLatency,
}

impl From<usize> for ResampleQuality {
    fn from(value: usize) -> Self {
        match value {
            0 => Self::VeryLow,
            1 => Self::Low,
            2 => Self::High,
            _ => Self::HighWithLowLatency,
        }
    }
}

/// The configuration for a [`PacketResampler`] or a [`Resampler`] create with
/// [`resampler_from_quality`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResamplerConfig {
    /// The quality of the resampling algorithm.
    ///
    /// By default this is set to [`ResampleQuality::High`].
    pub quality: ResampleQuality,

    /// The chunk size of the resampler. Lower values may reduce latency, but may
    /// use more CPU.
    ///
    /// By default this is set to `512`.
    pub chunk_size: usize,
}

impl Default for ResamplerConfig {
    fn default() -> Self {
        Self {
            quality: ResampleQuality::default(),
            chunk_size: 512,
        }
    }
}

/// Create a new [`Resampler`] with the given settings.
///
/// * `num_channels` - The number of audio channels.
/// * `in_sample_rate` - The sample rate of the input data.
/// * `out_sample_rate` - The sample rate of the output data.
/// * `config` - Extra configuration for the resampler.
///
/// # Panics
/// Panics if:
/// * `num_channels == 0`
/// * `in_sample_rate == 0`
/// * `out_sample_rate == 0`
/// * `config.chunk_size == 0`
/// * `config.sub_chunks == 0`
pub fn resampler_from_quality<T: Sample>(
    num_channels: usize,
    in_sample_rate: u32,
    out_sample_rate: u32,
    config: ResamplerConfig,
) -> Box<dyn Resampler<T>> {
    assert_ne!(num_channels, 0);
    assert_ne!(in_sample_rate, 0);
    assert_ne!(out_sample_rate, 0);
    assert_ne!(config.chunk_size, 0);

    let low = || -> Box<dyn rubato::Resampler<T>> {
        Box::new(
            rubato::Async::new_poly(
                out_sample_rate as f64 / in_sample_rate as f64,
                1.0,
                rubato::PolynomialDegree::Cubic,
                config.chunk_size,
                num_channels,
                rubato::FixedAsync::Input,
            )
            .unwrap(),
        )
    };

    match config.quality {
        ResampleQuality::VeryLow => Box::new(
            rubato::Async::new_poly(
                out_sample_rate as f64 / in_sample_rate as f64,
                1.0,
                rubato::PolynomialDegree::Linear,
                config.chunk_size,
                num_channels,
                rubato::FixedAsync::Input,
            )
            .unwrap(),
        ),
        ResampleQuality::Low => low(),
        ResampleQuality::High => {
            #[cfg(feature = "fft-resampler")]
            return Box::new(
                rubato::Fft::new(
                    in_sample_rate as usize,
                    out_sample_rate as usize,
                    config.chunk_size,
                    num_channels,
                    rubato::FixedSync::Input,
                )
                .unwrap(),
            );

            #[cfg(not(feature = "fft-resampler"))]
            return low();
        }
        ResampleQuality::HighWithLowLatency => Box::new(
            rubato::Async::new_sinc(
                out_sample_rate as f64 / in_sample_rate as f64,
                1.0,
                &rubato::SincInterpolationParameters::default(),
                config.chunk_size,
                num_channels,
                rubato::FixedAsync::Input,
            )
            .unwrap(),
        ),
    }
}

/// The resampling ratio for [`PacketResampler::from_custom`].
#[derive(Debug, Clone, Copy, PartialEq)]
enum ResampleRatio {
    IntegerSampleRate {
        in_sample_rate: u32,
        out_sample_rate: u32,
    },
    Float(f64),
}

/// A wrapper around rubato's [`Resampler`] that accepts inputs of any size and sends
/// resampled output packets to a given closure.
///
/// When using the [`Sequential`] [`PacketResamplerBuffer`], the output packets will
/// be in de-interleaved format (using &[`SequentialOwned`]). When using the
/// [`Interleaved`] [`PacketResamplerBuffer`], the output packets be be in interleaved
/// format (using `&[T]`).
///
/// This only supports synchronous resampling.
pub struct PacketResampler<T: Sample, B: PacketResamplerBuffer<T>> {
    resampler: Box<dyn Resampler<T>>,
    ratio: ResampleRatio,
    num_channels: usize,

    buffer: B,
    active_channels_mask: Option<Vec<bool>>,
    in_buf_len: usize,
    delay_frames_left: usize,
}

impl<T: Sample, B: PacketResamplerBuffer<T>> PacketResampler<T, B> {
    /// Create a new [`PacketResampler`].
    ///
    /// * `num_channels` - The number of audio channels.
    /// * `in_sample_rate` - The sample rate of the input data.
    /// * `out_sample_rate` - The sample rate of the output data.
    /// * `config` - Extra configuration for the resampler.
    ///
    /// # Panics
    /// Panics if:
    /// * `num_channels == 0`
    /// * `in_sample_rate == 0`
    /// * `out_sample_rate == 0`
    /// * `config.chunk_size == 0`
    /// * `config.sub_chunks == 0`
    pub fn new(
        num_channels: usize,
        in_sample_rate: u32,
        out_sample_rate: u32,
        config: ResamplerConfig,
    ) -> Self {
        let resampler =
            resampler_from_quality(num_channels, in_sample_rate, out_sample_rate, config);

        Self::new_inner(resampler, Some((in_sample_rate, out_sample_rate)))
    }

    /// Create a new [`PacketResampler`] using a custom [`Resampler`].
    ///
    /// This can be used, for example, to create a PacketResampler with non-integer input
    /// and/or output sample rates.
    pub fn from_custom(resampler: Box<dyn Resampler<T>>) -> Self {
        Self::new_inner(resampler, None)
    }

    fn new_inner(resampler: Box<dyn Resampler<T>>, sr: Option<(u32, u32)>) -> Self {
        let ratio = if let Some((in_sample_rate, out_sample_rate)) = sr {
            ResampleRatio::IntegerSampleRate {
                in_sample_rate,
                out_sample_rate,
            }
        } else {
            ResampleRatio::Float(resampler.resample_ratio())
        };

        let num_channels = resampler.nbr_channels();
        let input_frames_max = resampler.input_frames_max();
        let output_frames_max = resampler.output_frames_max();
        let delay_frames_left = resampler.output_delay();

        Self {
            resampler,
            ratio,
            num_channels,
            buffer: B::new(num_channels, input_frames_max, output_frames_max),
            active_channels_mask: Some(vec![false; num_channels]),
            in_buf_len: 0,
            delay_frames_left,
        }
    }

    /// The number of channels configured for this resampler.
    pub fn nbr_channels(&self) -> usize {
        self.num_channels
    }

    /// The resampling ratio `output / input`.
    pub fn ratio(&self) -> f64 {
        self.resampler.resample_ratio()
    }

    /// The number of frames (samples in a single channel of audio) that appear in
    /// a single packet of input data in the internal resampler.
    pub fn max_input_block_frames(&self) -> usize {
        self.resampler.input_frames_max()
    }

    /// The maximum number of frames (samples in a single channel of audio) that can
    /// appear in a single call to the `on_output_packet` closure in
    /// [`PacketResampler::process`].
    pub fn max_output_block_frames(&self) -> usize {
        self.resampler.output_frames_max()
    }

    /// The delay introduced by the internal resampler in number of output frames (
    /// samples in a single channel of audio).
    pub fn output_delay(&self) -> usize {
        self.resampler.output_delay()
    }

    /// The number of frames (samples in a single channel of audio) that are needed
    /// for an output buffer given the number of input frames.
    pub fn out_alloc_frames(&self, input_frames: u64) -> u64 {
        match self.ratio {
            // Use integer math when possible for more accurate results.
            ResampleRatio::IntegerSampleRate {
                in_sample_rate,
                out_sample_rate,
            } => {
                let out_frames = (input_frames * out_sample_rate as u64) / in_sample_rate as u64;
                let leftover_frames =
                    (input_frames * out_sample_rate as u64) % in_sample_rate as u64;

                let extra_frame = (leftover_frames as f64 / in_sample_rate as f64).round() as u64;

                out_frames + extra_frame
            }
            ResampleRatio::Float(ratio) => (input_frames as f64 * ratio).round() as u64,
        }
    }

    #[allow(unused)]
    pub(crate) fn tmp_input_frames(&self) -> usize {
        self.in_buf_len
    }

    /// Process the given input data and return packets of resampled output data.
    ///
    /// * `buffer_in` - The input data. You can use one of the types in the [`direct`]
    ///   module to wrap your input data into a type that implements [`Adapter`].
    /// * `input_range` - The range in each input channel to read from. If this is
    ///   `None`, then the entire input buffer will be read.
    /// * `active_channels_mask` - An optional mask that selects which channels in
    ///   `buffer_in` to use. Channels marked with `false` will be skipped and the
    ///   output channel filled with zeros. If `None`, then all of the channels will
    ///   be active.
    /// * `on_output_packet` - Gets called whenever there is a new packet of resampled
    ///   output data. `(buffer, output_frames)`
    ///     * When using [`Sequential`] output buffers, the output will be of type
    ///       &[`SequentialOwned`]. Note, the length of this buffer may be less than
    ///       `output_frames`. Only read up to `output_frames` data from the buffer.
    ///     * When using [`Interleaved`] output buffers, the output will be of type
    ///       `&[T]`. The number of frames in this slice will always be equal to
    ///       `output_frames`.
    /// * `last_packet` - If this is `Some`, then any leftover input samples in the
    ///   buffer will be flushed out and the resampler reset. Use this if this is the
    ///   last/only packet of input data.
    /// * `trim_delay` - If `true`, then the initial padded zeros introduced by the
    ///   internal resampler will be trimmed off.
    ///
    /// This method is realtime-safe.
    ///
    /// # Panics
    /// Panics if:
    /// * The `input_range` is out of bounds for any of the input channels.
    pub fn process(
        &mut self,
        buffer_in: &dyn Adapter<T>,
        input_range: Option<Range<usize>>,
        active_channels_mask: Option<&[bool]>,
        mut on_output_packet: impl FnMut(&B::Output, usize),
        last_packet: Option<LastPacketInfo>,
        trim_delay: bool,
    ) {
        let (input_start, total_input_frames) = if let Some(range) = input_range {
            (range.start, range.end - range.start)
        } else {
            (0, buffer_in.frames())
        };

        let use_indexing =
            active_channels_mask.is_some() || buffer_in.channels() < self.num_channels;

        let mut indexing = if use_indexing {
            let mut m = self.active_channels_mask.take().unwrap();

            if let Some(in_mask) = active_channels_mask {
                for (in_mask, out_mask) in in_mask.iter().zip(m.iter_mut()) {
                    *out_mask = *in_mask;
                }
            } else {
                for mask in m.iter_mut().take(buffer_in.channels()) {
                    *mask = true;
                }
            }
            for mask in m.iter_mut().skip(buffer_in.channels()) {
                *mask = false;
            }

            rubato::Indexing {
                input_offset: 0,
                output_offset: 0,
                partial_len: None,
                active_channels_mask: Some(m),
            }
        } else {
            rubato::Indexing {
                input_offset: 0,
                output_offset: 0,
                partial_len: None,
                active_channels_mask: None,
            }
        };

        let mut output_frames_processed: u64 = 0;

        let mut input_frames_left = total_input_frames;
        while input_frames_left > 0 {
            let needed_input_frames = self.resampler.input_frames_next();

            if self.in_buf_len < needed_input_frames {
                let block_frames_to_copy =
                    input_frames_left.min(needed_input_frames - self.in_buf_len);

                for ch_i in 0..self.num_channels {
                    let channel_active = ch_i < buffer_in.channels()
                        && active_channels_mask
                            .as_ref()
                            .map(|m| m.get(ch_i).copied().unwrap_or(false))
                            .unwrap_or(true);

                    if channel_active {
                        self.buffer.copy_from_other_to_input_channel(
                            buffer_in,
                            ch_i,
                            ch_i,
                            input_start + (total_input_frames - input_frames_left),
                            self.in_buf_len,
                            block_frames_to_copy,
                        );
                    }
                }

                self.in_buf_len += block_frames_to_copy;
                input_frames_left -= block_frames_to_copy;
            }

            if self.in_buf_len >= needed_input_frames {
                self.in_buf_len = 0;

                let (_, mut output_frames) = self
                    .buffer
                    .resample(Some(&indexing), &mut self.resampler)
                    .unwrap();

                if self.delay_frames_left > 0 {
                    if self.delay_frames_left >= output_frames {
                        self.delay_frames_left -= output_frames;

                        if trim_delay {
                            continue;
                        }
                    } else if trim_delay {
                        output_frames -= self.delay_frames_left;

                        self.buffer.output_copy_frames_within(
                            self.delay_frames_left,
                            0,
                            output_frames,
                        );

                        self.delay_frames_left = 0;
                    } else {
                        self.delay_frames_left = 0;
                    }
                }

                output_frames_processed += output_frames as u64;

                (on_output_packet)(self.buffer.output(output_frames), output_frames);
            }
        }

        if let Some(info) = &last_packet {
            let desired_output_frames = info
                .desired_output_frames
                .unwrap_or_else(|| output_frames_processed + self.resampler.output_delay() as u64);

            while output_frames_processed < desired_output_frames {
                indexing.partial_len = Some(self.in_buf_len);

                let (_, mut output_frames) = self
                    .buffer
                    .resample(Some(&indexing), &mut self.resampler)
                    .unwrap();

                self.in_buf_len = 0;

                if self.delay_frames_left > 0 {
                    if self.delay_frames_left >= output_frames {
                        self.delay_frames_left -= output_frames;

                        if trim_delay {
                            continue;
                        }
                    } else if trim_delay {
                        output_frames -= self.delay_frames_left;

                        self.buffer.output_copy_frames_within(
                            self.delay_frames_left,
                            0,
                            output_frames,
                        );

                        self.delay_frames_left = 0;
                    } else {
                        self.delay_frames_left = 0;
                    }
                }

                output_frames =
                    output_frames.min((desired_output_frames - output_frames_processed) as usize);
                output_frames_processed += output_frames as u64;

                (on_output_packet)(self.buffer.output(output_frames), output_frames);
            }

            self.reset();
        }

        if let Some(m) = indexing.active_channels_mask.take() {
            self.active_channels_mask = Some(m);
        }
    }

    pub fn output_delay_frames_left(&self) -> usize {
        self.delay_frames_left
    }

    pub fn reset(&mut self) {
        self.resampler.reset();
        self.in_buf_len = 0;
        self.delay_frames_left = self.resampler.output_delay();
    }

    pub fn into_inner(self) -> Box<dyn Resampler<T>> {
        self.resampler
    }
}

/// Options for processes the last packet in a resampler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LastPacketInfo {
    /// The desired number of output frames that should be sent via the
    /// `on_output_packet` closure.
    ///
    /// If this is `None`, then the last packet sent may contain extra
    /// padded zeros on the end.
    pub desired_output_frames: Option<u64>,
}

/// The type of output buffer to use for a [`PacketResampler`].
///
/// The provided options are [`Sequential`] and [`Interleaved`].
pub trait PacketResamplerBuffer<T: Sample> {
    type Output: ?Sized;

    fn new(channels: usize, input_frames: usize, output_frames: usize) -> Self;

    fn output(&self, frames: usize) -> &Self::Output;

    fn resample(
        &mut self,
        indexing: Option<&rubato::Indexing>,
        resampler: &mut Box<dyn Resampler<T>>,
    ) -> ResampleResult<(usize, usize)>;

    /// Copy values from a channel of another Adapter.
    /// The `self_skip` and `other_skip` arguments are the offsets
    /// in frames for where copying starts in the two channels.
    /// The method copies `take` values.
    ///
    /// Returns the the number of values that were clipped during conversion.
    /// Implementations that do not perform any conversion
    /// always return zero clipped samples.
    ///
    /// If an invalid channel number is given,
    /// or if either of the channels is to short to copy `take` values,
    /// no values will be copied and `None` is returned.
    fn copy_from_other_to_input_channel(
        &mut self,
        other: &dyn Adapter<T>,
        other_channel: usize,
        self_channel: usize,
        other_skip: usize,
        self_skip: usize,
        take: usize,
    ) -> Option<usize>;

    /// Write the provided value to every sample in a range of frames.
    /// Can be used to clear a range of frames by writing zeroes,
    /// or to initialize each sample to a certain value.
    /// Returns `None` if called with a too large range.
    fn input_fill_frames_with(&mut self, start: usize, count: usize, value: &T) -> Option<usize>;

    /// Write the provided value to every sample in the entire buffer.
    /// Can be used to clear a buffer by writing zeroes,
    /// or to initialize each sample to a certain value.
    fn input_fill_with(&mut self, value: &T);

    /// Copy frames within the buffer.
    /// Copying is performed for all channels.
    /// Copies `count` frames, from the range `src..src+count`,
    /// to the range `dest..dest+count`.
    /// The two regions are allowed to overlap.
    /// The default implementation copies by calling the read and write methods,
    /// while type specific implementations can use more efficient methods.
    fn output_copy_frames_within(&mut self, src: usize, dest: usize, count: usize);
}

/// Use de-interleaved output packets for a [`PacketResampler`].
pub struct Sequential<T: Sample> {
    in_buffer: SequentialOwned<T>,
    out_buffer: SequentialOwned<T>,
}

impl<T: Sample> PacketResamplerBuffer<T> for Sequential<T> {
    type Output = SequentialOwned<T>;

    fn new(channels: usize, input_frames: usize, output_frames: usize) -> Self {
        Self {
            in_buffer: SequentialOwned::new(T::zero(), channels, input_frames),
            out_buffer: SequentialOwned::new(T::zero(), channels, output_frames),
        }
    }

    fn output(&self, _frames: usize) -> &Self::Output {
        &self.out_buffer
    }

    fn resample(
        &mut self,
        indexing: Option<&rubato::Indexing>,
        resampler: &mut Box<dyn Resampler<T>>,
    ) -> ResampleResult<(usize, usize)> {
        resampler.process_into_buffer(&self.in_buffer, &mut self.out_buffer, indexing)
    }

    fn copy_from_other_to_input_channel(
        &mut self,
        other: &dyn Adapter<T>,
        other_channel: usize,
        self_channel: usize,
        other_skip: usize,
        self_skip: usize,
        take: usize,
    ) -> Option<usize> {
        self.in_buffer.copy_from_other_to_channel(
            other,
            other_channel,
            self_channel,
            other_skip,
            self_skip,
            take,
        )
    }

    fn input_fill_frames_with(&mut self, start: usize, count: usize, value: &T) -> Option<usize> {
        self.in_buffer.fill_frames_with(start, count, value)
    }

    fn input_fill_with(&mut self, value: &T) {
        self.in_buffer.fill_with(value);
    }

    fn output_copy_frames_within(&mut self, src: usize, dest: usize, count: usize) {
        self.out_buffer.copy_frames_within(src, dest, count);
    }
}

/// Use interleaved output packets for a [`PacketResampler`].
pub struct Interleaved<T: Sample> {
    in_buffer: InterleavedOwned<T>,
    out_buffer: Vec<T>,
    channels: usize,
    output_frames: usize,
}

impl<T: Sample> PacketResamplerBuffer<T> for Interleaved<T> {
    type Output = [T];

    fn new(channels: usize, input_frames: usize, output_frames: usize) -> Self {
        let out_buffer_size = output_frames * channels;
        let mut out_buffer = Vec::new();
        out_buffer.reserve_exact(out_buffer_size);
        out_buffer.resize(out_buffer_size, T::zero());

        Self {
            in_buffer: InterleavedOwned::new(T::zero(), channels, input_frames),
            out_buffer,
            channels,
            output_frames,
        }
    }

    fn output(&self, frames: usize) -> &Self::Output {
        &self.out_buffer[0..frames * self.channels]
    }

    fn resample(
        &mut self,
        indexing: Option<&rubato::Indexing>,
        resampler: &mut Box<dyn Resampler<T>>,
    ) -> ResampleResult<(usize, usize)> {
        let mut out_buffer_wrapper = direct::InterleavedSlice::new_mut(
            &mut self.out_buffer,
            self.channels,
            self.output_frames,
        )
        .unwrap();

        resampler.process_into_buffer(&self.in_buffer, &mut out_buffer_wrapper, indexing)
    }

    fn copy_from_other_to_input_channel(
        &mut self,
        other: &dyn Adapter<T>,
        other_channel: usize,
        self_channel: usize,
        other_skip: usize,
        self_skip: usize,
        take: usize,
    ) -> Option<usize> {
        self.in_buffer.copy_from_other_to_channel(
            other,
            other_channel,
            self_channel,
            other_skip,
            self_skip,
            take,
        )
    }

    fn input_fill_frames_with(&mut self, start: usize, count: usize, value: &T) -> Option<usize> {
        self.in_buffer.fill_frames_with(start, count, value)
    }

    fn input_fill_with(&mut self, value: &T) {
        self.in_buffer.fill_with(value);
    }

    fn output_copy_frames_within(&mut self, src: usize, dest: usize, count: usize) {
        self.out_buffer.copy_within(
            src * self.channels..count * self.channels,
            dest * self.channels,
        );
    }
}

/// Extend the given Vec with the contents of a channel from the given [`Adapter`].
///
/// If the adapter does not contain enough frames, then only the available number of
/// frames will be appended to the Vec.
///
/// Returns the number of frames that were appended to the Vec.
///
/// # Panics
/// * Panics if `buffer_in_channel >= buffer_in.channels()`
pub fn extend_from_adapter_channel<T: Sample>(
    out_buffer: &mut Vec<T>,
    buffer_in: &dyn Adapter<T>,
    buffer_in_skip: usize,
    buffer_in_channel: usize,
    frames: usize,
) -> usize {
    assert!(buffer_in_channel < buffer_in.channels());

    let out_buffer_len = out_buffer.len();
    let available = out_buffer.capacity() - out_buffer_len;
    if available < frames {
        out_buffer.reserve(frames);
    }

    // Safety:
    // * We ensured that the output buffer has enough capacity above.
    // * All the new frames in the output buffer will be filled with data below, and
    // we correctly truncate any frames which did not get filled with data.
    unsafe {
        out_buffer.set_len(out_buffer_len + frames);
    }

    let frames_copied = buffer_in.copy_from_channel_to_slice(
        buffer_in_channel,
        buffer_in_skip,
        &mut out_buffer[out_buffer_len..out_buffer_len + frames],
    );

    // Truncate any unused data.
    if frames_copied < frames {
        // Safety:
        // * The new length is less than the old length.
        // * T requires `Copy`, so there is no need to call the destructor on each sample.
        unsafe {
            out_buffer.set_len(out_buffer_len + frames_copied);
        }
    }

    frames_copied
}
