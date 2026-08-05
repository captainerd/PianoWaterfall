use std::{
    ops::Range,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use audioadapter::{Adapter, AdapterMut};
use audioadapter_buffers::direct::InterleavedSlice;
use ringbuf::traits::{Consumer, Observer, Producer, Split};
#[cfg(feature = "resampler")]
use rubato::Resampler;

use crate::Sample;

#[cfg(feature = "resampler")]
use crate::{Interleaved, PacketResampler, ResamplerConfig};

const TMP_OUT_BUFFER_FRAMES: usize = 512;

/// Additional options for a resampling channel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResamplingChannelConfig {
    /// The amount of latency added in seconds between the input stream and the
    /// output stream. If this value is too small, then underflows may occur.
    ///
    /// The default value is `0.15` (150 ms).
    pub latency_seconds: f64,

    /// The capacity of the channel in seconds. If this is too small, then
    /// overflows may occur. This should be at least twice as large as
    /// `latency_seconds`.
    ///
    /// Note, the actual capacity may be slightly smaller due to how the internal
    /// sampler processes in chunks.
    ///
    /// The default value is `0.4` (400 ms).
    pub capacity_seconds: f64,

    /// If the number of occupied samples in the channel is greater than or equal to
    /// (`latency_seconds + percent * (capacity_seconds - latency_seconds)`), then discard the
    /// number of samples needed to bring the number of occupied seconds back down to
    /// [`ResamplingChannelConfig::latency_seconds`]. This is used to avoid excessive
    /// overflows and reduce the percieved audio glitchiness.
    ///
    /// The percentage is a value in the range `[0.0, 100.0]`.
    ///
    /// Set to `None` to disable this autocorrecting behavior. If the producer end is being
    /// used in a non-realtime context, then this should be set to `None`.
    ///
    /// By default this is set to `Some(75.0)`.
    pub overflow_autocorrect_percent_threshold: Option<f64>,

    /// If the number of occupied samples in the channel is below or equal to the given
    /// percentage of [`ResamplingChannelConfig::latency_seconds`], then insert the number of
    /// zero frames needed to bring the number of occupied samples back up to
    /// [`ResamplingChannelConfig::latency_seconds`]. This is used to avoid excessive underflows
    /// and reduce the percieved audio glitchiness.
    ///
    /// The percentage is a value in the range `[0.0, 100.0]`.
    ///
    /// Set to `None` to disable this autocorrecting behavior. If the consumer end is being
    /// used in a non-realtime context, then this should be set to `None`.
    ///
    /// By default this is set to `Some(25.0)`.
    pub underflow_autocorrect_percent_threshold: Option<f64>,

    #[cfg(feature = "resampler")]
    /// The configuration of the resampler to use.
    ///
    /// This is ignored when using [`resampling_channel_custom`].
    pub resampler_config: ResamplerConfig,

    #[cfg(feature = "resampler")]
    /// If `true`, then the delay of the internal resampler (if used) will be
    /// subtracted from the `latency_seconds` value to keep the perceived
    /// latency consistent.
    ///
    /// The default value is `true`.
    pub subtract_resampler_delay: bool,
}

impl Default for ResamplingChannelConfig {
    fn default() -> Self {
        Self {
            latency_seconds: 0.15,
            capacity_seconds: 0.4,
            overflow_autocorrect_percent_threshold: Some(75.0),
            underflow_autocorrect_percent_threshold: Some(25.0),
            #[cfg(feature = "resampler")]
            resampler_config: ResamplerConfig::default(),
            #[cfg(feature = "resampler")]
            subtract_resampler_delay: true,
        }
    }
}

/// Create a new realtime-safe spsc channel for sending samples across streams.
///
/// If the input and output samples rates differ, then this will automatically
/// resample the input stream to match the output stream (unless the "resample"
/// feature is disabled). If the sample rates match, then no resampling will
/// occur.
///
/// Internally this uses the `rubato` and `ringbuf` crates.
///
/// * `in_sample_rate` - The sample rate of the input stream.
/// * `out_sample_rate` - The sample rate of the output stream.
/// * `num_channels` - The number of channels in the stream.
/// * `push_interleave_only` - If you are NOT using [`ResamplingProd::push`],
///   then you can set this to `true` to save a bit of memory. Otherwise,
///   set this to `false`.
/// * `config` - Additional options for the resampling channel.
///
/// # Panics
///
/// Panics when any of the following are true:
///
/// * `in_sample_rate == 0`
/// * `out_sample_rate == 0`
/// * `num_channels == 0`
/// * `config.latency_seconds <= 0.0`
/// * `config.capacity_seconds <= 0.0`
/// * `config.output_chunk_size == 0`
///
/// If the "resampler" feature is disabled, then this will also panic if
/// `in_sample_rate != out_sample_rate`.
pub fn resampling_channel<T: Sample>(
    num_channels: usize,
    in_sample_rate: u32,
    out_sample_rate: u32,
    push_interleave_only: bool,
    config: ResamplingChannelConfig,
) -> (ResamplingProd<T>, ResamplingCons<T>) {
    #[cfg(not(feature = "resampler"))]
    assert_eq!(
        in_sample_rate, out_sample_rate,
        "Input and output sample rate must be equal when the \"resampler\" feature is disabled"
    );

    #[cfg(feature = "resampler")]
    let resampler = if in_sample_rate != out_sample_rate {
        Some(PacketResampler::<T, Interleaved<T>>::new(
            num_channels,
            in_sample_rate,
            out_sample_rate,
            config.resampler_config,
        ))
    } else {
        None
    };

    resampling_channel_inner(
        #[cfg(feature = "resampler")]
        resampler,
        num_channels,
        in_sample_rate as f64,
        Some(out_sample_rate),
        config,
        push_interleave_only,
    )
}

/// Create a new realtime-safe spsc channel for sending samples across streams
/// using the custom resampler.
///
/// Internally this uses the `rubato` and `ringbuf` crates.
///
/// * `resampler` - The custom rubato resampler.
/// * `num_channels` - The number of channels in the stream.
/// * `in_sample_rate` - The sample rate of the input signal. This does not
///   have to be an integer.
/// * `push_interleave_only` - If you are NOT using [`ResamplingProd::push`],
///   then you can set this to `true` to save a bit of memory. Otherwise,
///   set this to `false`.
/// * `config` - Additional options for the resampling channel. Note that
///   `config.quality` will be ignored.
///
/// # Panics
///
/// Panics when any of the following are true:
///
/// * `num_channels == 0`
/// * `num_channels > resampler.nbr_channels()`
/// * `in_sample_rate <= 0.0`
/// * `config.latency_seconds <= 0.0`
/// * `config.capacity_seconds <= 0.0`
#[cfg(feature = "resampler")]
pub fn resampling_channel_custom<T: Sample>(
    resampler: Box<dyn Resampler<T>>,
    num_channels: usize,
    in_sample_rate: f64,
    push_interleave_only: bool,
    config: ResamplingChannelConfig,
) -> (ResamplingProd<T>, ResamplingCons<T>) {
    assert_ne!(num_channels, 0);
    assert!(num_channels <= resampler.nbr_channels());
    assert!(in_sample_rate.is_finite());
    assert!(in_sample_rate > 0.0);

    let resampler = Some(PacketResampler::from_custom(resampler));

    resampling_channel_inner(
        resampler,
        num_channels,
        in_sample_rate,
        None,
        config,
        push_interleave_only,
    )
}

fn resampling_channel_inner<T: Sample>(
    #[cfg(feature = "resampler")] resampler: Option<PacketResampler<T, Interleaved<T>>>,
    num_channels: usize,
    in_sample_rate: f64,
    out_sample_rate: Option<u32>,
    config: ResamplingChannelConfig,
    push_interleave_only: bool,
) -> (ResamplingProd<T>, ResamplingCons<T>) {
    assert!(config.latency_seconds > 0.0);
    assert!(config.capacity_seconds > 0.0);

    #[cfg(feature = "resampler")]
    let output_to_input_ratio = resampler.as_ref().map(|r| r.ratio()).unwrap_or(1.0);
    #[cfg(not(feature = "resampler"))]
    let output_to_input_ratio = 1.0;

    #[cfg(feature = "resampler")]
    let is_resampling = resampler.is_some();
    #[cfg(feature = "resampler")]
    let resampler_output_delay = resampler.as_ref().map(|r| r.output_delay()).unwrap_or(0);

    let in_sample_rate_recip = in_sample_rate.recip();
    let out_sample_rate = out_sample_rate
        .map(|o| o as f64)
        .unwrap_or_else(|| in_sample_rate * output_to_input_ratio);
    let out_sample_rate_recip = out_sample_rate.recip();

    let latency_frames = ((out_sample_rate * config.latency_seconds).round() as usize).max(1);

    #[allow(unused_mut)]
    let mut channel_latency_frames = latency_frames;

    #[cfg(feature = "resampler")]
    if resampler.is_some() && config.subtract_resampler_delay {
        if latency_frames > resampler_output_delay {
            channel_latency_frames -= resampler_output_delay;
        } else {
            channel_latency_frames = 1;
        }
    }

    let channel_latency_samples = channel_latency_frames * num_channels;

    let buffer_capacity_frames = ((in_sample_rate * config.capacity_seconds).round() as usize)
        .max(channel_latency_frames * 2);

    let (mut prod, cons) = ringbuf::HeapRb::<T>::new(buffer_capacity_frames * num_channels).split();

    // Fill the channel with initial zeros to create the desired latency.
    prod.push_slice(&vec![T::zero(); channel_latency_frames * num_channels]);

    let shared_state = Arc::new(SharedState::new());

    let overflow_autocorrect_threshold_samples =
        config
            .overflow_autocorrect_percent_threshold
            .map(|percent| {
                let range_samples =
                    (buffer_capacity_frames - channel_latency_frames) * num_channels;

                ((range_samples as f64 * (percent / 100.0).clamp(0.0, 1.0)).round() as usize)
                    .min(range_samples)
                    + channel_latency_samples
            });
    let underflow_autocorrect_threshold_samples = config
        .underflow_autocorrect_percent_threshold
        .map(|percent| {
            ((channel_latency_samples as f64 * (percent / 100.0).clamp(0.0, 1.0)).round() as usize)
                .min(channel_latency_samples)
        });

    #[cfg(feature = "resampler")]
    let do_create_tmp_out_buffer = !push_interleave_only && resampler.is_none();

    #[cfg(not(feature = "resampler"))]
    let do_create_tmp_out_buffer = !push_interleave_only;

    let tmp_out_buffer = do_create_tmp_out_buffer.then(|| {
        let len = TMP_OUT_BUFFER_FRAMES * num_channels;
        let mut v = Vec::new();
        v.reserve_exact(len);
        v.resize(len, T::zero());
        v
    });

    (
        ResamplingProd {
            prod,
            num_channels,
            latency_seconds: config.latency_seconds,
            channel_latency_samples,
            in_sample_rate,
            in_sample_rate_recip,
            out_sample_rate,
            out_sample_rate_recip,
            shared_state: Arc::clone(&shared_state),
            waiting_for_output_to_reset: false,
            underflow_autocorrect_threshold_samples,
            tmp_out_buffer,
            #[cfg(feature = "resampler")]
            resampler,
            #[cfg(feature = "resampler")]
            output_to_input_ratio,
        },
        ResamplingCons {
            cons,
            num_channels,
            latency_frames,
            latency_seconds: config.latency_seconds,
            channel_latency_samples,
            in_sample_rate,
            out_sample_rate,
            out_sample_rate_recip,
            shared_state,
            waiting_for_input_to_reset: false,
            overflow_autocorrect_threshold_samples,
            #[cfg(feature = "resampler")]
            is_resampling,
            #[cfg(feature = "resampler")]
            resampler_output_delay,
        },
    )
}

/// The producer end of a realtime-safe spsc channel for sending samples across
/// streams.
///
/// If the input and output samples rates differ, then this will automatically
/// resample the input stream to match the output stream. If the sample rates
/// match, then no resampling will occur.
///
/// Internally this uses the `rubato` and `ringbuf` crates.
pub struct ResamplingProd<T: Sample> {
    prod: ringbuf::HeapProd<T>,
    num_channels: usize,
    latency_seconds: f64,
    channel_latency_samples: usize,
    in_sample_rate: f64,
    in_sample_rate_recip: f64,
    out_sample_rate: f64,
    out_sample_rate_recip: f64,
    shared_state: Arc<SharedState>,
    waiting_for_output_to_reset: bool,
    underflow_autocorrect_threshold_samples: Option<usize>,
    tmp_out_buffer: Option<Vec<T>>,

    #[cfg(feature = "resampler")]
    resampler: Option<PacketResampler<T, Interleaved<T>>>,
    #[cfg(feature = "resampler")]
    output_to_input_ratio: f64,
}

impl<T: Sample + 'static> ResamplingProd<T> {
    /// Push the given input data.
    ///
    /// * `input` - The input data. You can use one of the types in the
    ///   [`audioadapter_buffers::direct`](crate::audioadapter_buffers::direct) module
    ///   to wrap your input data into a type that implements [`Adapter`].
    /// * `input_range` - The range in each input channel to read from. If this is
    ///   `None`, then the entire input buffer will be read.
    /// * `active_channels_mask` - An optional mask that selects which channels in
    ///   `input` to use. Channels marked with `false` will be skipped and that
    ///   output channel filled with zeros. If `None`, then all of the channels will
    ///   be active.
    ///
    /// This method is realtime-safe.
    ///
    /// # Panics
    /// Panics if:
    /// * The `input_range` is out of bounds for any of the input channels.
    /// * This producer was created with `push_interleave_only` set to `true`.
    pub fn push(
        &mut self,
        input: &dyn Adapter<T>,
        input_range: Option<Range<usize>>,
        active_channels_mask: Option<&[bool]>,
    ) -> PushStatus {
        self.set_input_stream_ready(true);

        if !self.output_stream_ready() {
            return PushStatus::OutputNotReady;
        }

        self.poll_reset();

        let (input_start, total_frames) = if let Some(range) = input_range {
            (range.start, range.end - range.start)
        } else {
            (0, input.frames())
        };

        let available_frames = self.available_frames();
        let total_frames_to_copy = total_frames.min(available_frames);

        #[cfg(feature = "resampler")]
        let process_non_resampled = self.resampler.is_none();
        #[cfg(not(feature = "resampler"))]
        let process_non_resampled = true;

        #[cfg(feature = "resampler")]
        if let Some(resampler) = &mut self.resampler {
            resampler.process(
                input,
                Some(input_start..input_start + total_frames_to_copy),
                active_channels_mask,
                |output_packet, _frames| {
                    let pushed_samples = self.prod.push_slice(output_packet);
                    debug_assert_eq!(pushed_samples, output_packet.len());
                },
                None,
                false,
            );
        }

        if process_non_resampled {
            let tmp_out_buffer = self.tmp_out_buffer.as_mut().expect(
                "ResamplingProd::push was called even though push_interleave_only was set to true",
            );

            if input.channels() < self.num_channels || active_channels_mask.is_some() {
                let tmp_out_buf_len = tmp_out_buffer.len();
                tmp_out_buffer[0..(total_frames_to_copy * self.num_channels).min(tmp_out_buf_len)]
                    .fill(T::zero());
            }

            let mut frames_left = total_frames_to_copy;
            while frames_left > 0 {
                let block_frames_to_copy = frames_left.min(TMP_OUT_BUFFER_FRAMES);

                {
                    let mut out_buf_wrapper = InterleavedSlice::new_mut(
                        tmp_out_buffer,
                        self.num_channels,
                        TMP_OUT_BUFFER_FRAMES,
                    )
                    .unwrap();

                    for ch_i in 0..input.channels() {
                        let channel_active = active_channels_mask
                            .as_ref()
                            .map(|m| m.get(ch_i).copied().unwrap_or(false))
                            .unwrap_or(true);

                        if channel_active {
                            out_buf_wrapper.copy_from_other_to_channel(
                                input,
                                ch_i,
                                ch_i,
                                input_start + (total_frames_to_copy - frames_left),
                                0,
                                block_frames_to_copy,
                            );
                        }
                    }
                }

                let pushed_samples = self
                    .prod
                    .push_slice(&tmp_out_buffer[0..block_frames_to_copy * self.num_channels]);
                debug_assert_eq!(pushed_samples, block_frames_to_copy * self.num_channels);

                frames_left -= block_frames_to_copy;
            }
        }

        if total_frames_to_copy < total_frames {
            PushStatus::OverflowOccurred {
                num_frames_pushed: total_frames_to_copy,
            }
        } else if let Some(zero_frames_pushed) = self.autocorrect_underflows() {
            PushStatus::UnderflowCorrected {
                num_zero_frames_pushed: zero_frames_pushed,
            }
        } else {
            PushStatus::Ok
        }
    }

    /// Push the given input data in interleaved format.
    ///
    /// The input buffer must have the same number of channels as this producer.
    ///
    /// This method is realtime-safe.
    pub fn push_interleaved(&mut self, input: &[T]) -> PushStatus {
        self.set_input_stream_ready(true);

        if !self.output_stream_ready() {
            return PushStatus::OutputNotReady;
        }

        self.poll_reset();

        let total_frames = input.len() / self.num_channels;

        let available_frames = self.available_frames();
        let total_frames_to_copy = total_frames.min(available_frames);

        #[cfg(feature = "resampler")]
        let process_non_resampled = self.resampler.is_none();
        #[cfg(not(feature = "resampler"))]
        let process_non_resampled = true;

        #[cfg(feature = "resampler")]
        if let Some(resampler) = &mut self.resampler {
            let input_wrapper =
                InterleavedSlice::new(input, self.num_channels, total_frames_to_copy).unwrap();

            resampler.process(
                &input_wrapper,
                None,
                None,
                |output_packet, _frames| {
                    let pushed_samples = self.prod.push_slice(output_packet);
                    debug_assert_eq!(pushed_samples, output_packet.len());
                },
                None,
                false,
            );
        }

        if process_non_resampled {
            let pushed_samples = self
                .prod
                .push_slice(&input[0..total_frames_to_copy * self.num_channels]);
            debug_assert_eq!(pushed_samples, total_frames_to_copy * self.num_channels);
        }

        if total_frames_to_copy < total_frames {
            PushStatus::OverflowOccurred {
                num_frames_pushed: total_frames_to_copy,
            }
        } else if let Some(zero_frames_pushed) = self.autocorrect_underflows() {
            PushStatus::UnderflowCorrected {
                num_zero_frames_pushed: zero_frames_pushed,
            }
        } else {
            PushStatus::Ok
        }
    }

    /// Returns the number of input frames (samples in a single channel of audio)
    /// that are currently available to be pushed to the channel.
    ///
    /// If the output stream is not ready yet, then this will return `0`.
    ///
    /// This method is realtime-safe.
    pub fn available_frames(&mut self) -> usize {
        if !self.output_stream_ready() {
            return 0;
        }

        self.poll_reset();

        let output_vacant_frames = self.prod.vacant_len() / self.num_channels;

        #[cfg(feature = "resampler")]
        if let Some(resampler) = &self.resampler {
            let mut input_vacant_frames =
                (output_vacant_frames as f64 * self.output_to_input_ratio).floor() as usize;

            // Give some leeway to account for floating point inaccuracies.
            input_vacant_frames = input_vacant_frames.saturating_sub(1);

            if input_vacant_frames < resampler.max_input_block_frames() {
                return 0;
            }

            // The resampler processes in chunks.
            input_vacant_frames = (input_vacant_frames / resampler.max_input_block_frames())
                * resampler.max_input_block_frames();

            return input_vacant_frames - resampler.tmp_input_frames();
        }

        output_vacant_frames
    }

    /// The amount of data in seconds that is available to be pushed to the
    /// channel.
    ///
    /// If the output stream is not ready yet, then this will return `0.0`.
    ///
    /// This method is realtime-safe.
    pub fn available_seconds(&mut self) -> f64 {
        self.available_frames() as f64 * self.in_sample_rate_recip
    }

    /// The amount of data that is currently occupied in the channel, in units of
    /// output frames (samples in a single channel of audio).
    ///
    /// Note, this is the number of frames in the *output* audio stream, not the
    /// input audio stream.
    ///
    /// This method is realtime-safe.
    pub fn occupied_output_frames(&self) -> usize {
        self.prod.occupied_len() / self.num_channels
    }

    /// The amount of data that is currently occupied in the channel, in units of
    /// seconds.
    ///
    /// This method is realtime-safe.
    pub fn occupied_seconds(&self) -> f64 {
        self.occupied_output_frames() as f64 * self.out_sample_rate_recip
    }

    /// The number of channels configured for this stream.
    ///
    /// This method is realtime-safe.
    pub fn num_channels(&self) -> usize {
        self.num_channels
    }

    /// The sample rate of the input stream.
    ///
    /// This method is realtime-safe.
    pub fn in_sample_rate(&self) -> f64 {
        self.in_sample_rate
    }

    /// The sample rate of the output stream.
    ///
    /// This method is realtime-safe.
    pub fn out_sample_rate(&self) -> f64 {
        self.out_sample_rate
    }

    /// The latency of the channel in units of seconds.
    ///
    /// This method is realtime-safe.
    pub fn latency_seconds(&self) -> f64 {
        self.latency_seconds
    }

    /// Returns `true` if this channel is currently resampling.
    ///
    /// This method is realtime-safe.
    #[cfg(feature = "resampler")]
    pub fn is_resampling(&self) -> bool {
        self.resampler.is_some()
    }

    /// Tell the consumer to clear all queued frames in the buffer.
    ///
    /// This method is realtime-safe.
    pub fn reset(&mut self) {
        self.shared_state.reset.store(true, Ordering::Relaxed);

        self.waiting_for_output_to_reset = true;

        #[cfg(feature = "resampler")]
        if let Some(resampler) = &mut self.resampler {
            resampler.reset();
        }
    }

    /// Manually notify the output stream that the input stream is ready/not ready
    /// to push samples to the channel.
    ///
    /// If this producer end is being used in a non-realtime context, then it is
    /// a good idea to set this to `true` so that the consumer end can start
    /// reading samples from the channel immediately.
    ///
    /// Note, calling [`ResamplingProd::push`] and
    /// [`ResamplingProd::push_interleaved`] automatically sets the input stream as
    /// ready.
    ///
    /// This method is realtime-safe.
    pub fn set_input_stream_ready(&mut self, ready: bool) {
        self.shared_state
            .input_stream_ready
            .store(ready, Ordering::Relaxed);
    }

    /// Whether or not the output stream is ready to read samples from the channel.
    ///
    /// This method is realtime-safe.
    pub fn output_stream_ready(&self) -> bool {
        self.shared_state
            .output_stream_ready
            .load(Ordering::Relaxed)
            && !self.shared_state.reset.load(Ordering::Relaxed)
    }

    /// Correct for any underflows.
    ///
    /// This returns the number of extra zero frames (samples in a single channel of audio)
    /// that were added due to an underflow occurring. If no underflow occured, then `None`
    /// is returned.
    ///
    /// Note, this method is already automatically called in [`ResamplingProd::push`] and
    /// [`ResamplingProd::push_interleaved`].
    ///
    /// This will have no effect if [`ResamplingChannelConfig::underflow_autocorrect_percent_threshold`]
    /// was set to `None`.
    ///
    /// This method is realtime-safe.
    pub fn autocorrect_underflows(&mut self) -> Option<usize> {
        if !self.output_stream_ready() {
            return None;
        }

        self.poll_reset();

        if let Some(underflow_autocorrect_threshold_samples) =
            self.underflow_autocorrect_threshold_samples
        {
            let len = self.prod.occupied_len();

            if len <= underflow_autocorrect_threshold_samples && len < self.channel_latency_samples
            {
                let correction_samples = self.channel_latency_samples - len;

                self.prod
                    .push_iter((0..correction_samples).map(|_| T::zero()));

                return Some(correction_samples / self.num_channels);
            }
        }

        None
    }

    fn poll_reset(&mut self) {
        if self.waiting_for_output_to_reset {
            self.waiting_for_output_to_reset = false;

            // Fill the channel with initial zeros to create the desired latency.
            self.prod
                .push_iter((0..self.channel_latency_samples).map(|_| T::zero()));
        }
    }
}

/// The consumer end of a realtime-safe spsc channel for sending samples across
/// streams.
///
/// If the input and output samples rates differ, then this will automatically
/// resample the input stream to match the output stream. If the sample rates
/// match, then no resampling will occur.
///
/// Internally this uses the `rubato` and `ringbuf` crates.
pub struct ResamplingCons<T: Sample> {
    cons: ringbuf::HeapCons<T>,
    num_channels: usize,
    latency_seconds: f64,
    latency_frames: usize,
    channel_latency_samples: usize,
    in_sample_rate: f64,
    out_sample_rate: f64,
    out_sample_rate_recip: f64,
    shared_state: Arc<SharedState>,
    waiting_for_input_to_reset: bool,
    overflow_autocorrect_threshold_samples: Option<usize>,

    #[cfg(feature = "resampler")]
    resampler_output_delay: usize,
    #[cfg(feature = "resampler")]
    is_resampling: bool,
}

impl<T: Sample + 'static> ResamplingCons<T> {
    /// The number of channels configured for this stream.
    ///
    /// This method is realtime-safe.
    pub fn num_channels(&self) -> usize {
        self.num_channels
    }

    /// The sample rate of the input stream.
    ///
    /// This method is realtime-safe.
    pub fn in_sample_rate(&self) -> f64 {
        self.in_sample_rate
    }

    /// The sample rate of the output stream.
    ///
    /// This method is realtime-safe.
    pub fn out_sample_rate(&self) -> f64 {
        self.out_sample_rate
    }

    /// The latency of the channel in units of seconds.
    ///
    /// This method is realtime-safe.
    pub fn latency_seconds(&self) -> f64 {
        self.latency_seconds
    }

    /// The latency of the channel in units of output frames.
    ///
    /// This method is realtime-safe.
    pub fn latency_frames(&self) -> usize {
        self.latency_frames
    }

    /// The number of frames (samples in a single channel of audio) that are
    /// currently available to be read from the channel.
    ///
    /// If the input stream is not ready yet, then this will return `0`.
    ///
    /// This method is realtime-safe.
    pub fn available_frames(&self) -> usize {
        if self.input_stream_ready() {
            self.cons.occupied_len() / self.num_channels
        } else {
            0
        }
    }

    /// The amount of data in seconds that is currently available to be read
    /// from the channel.
    ///
    /// If the input stream is not ready yet, then this will return `0.0`.
    ///
    /// This method is realtime-safe.
    pub fn available_seconds(&self) -> f64 {
        self.available_frames() as f64 * self.out_sample_rate_recip
    }

    /// The amount of data that is currently occupied in the channel, in units of
    /// seconds.
    ///
    /// This method is realtime-safe.
    pub fn occupied_seconds(&self) -> f64 {
        (self.cons.occupied_len() / self.num_channels) as f64 * self.out_sample_rate_recip
    }

    /// Returns `true` if this channel is currently resampling.
    ///
    /// This method is realtime-safe.
    #[cfg(feature = "resampler")]
    pub fn is_resampling(&self) -> bool {
        self.is_resampling
    }

    /// The delay of the internal resampler in number of output frames (samples in
    /// a single channel of audio).
    ///
    /// If there is no resampler being used for this channel, then this will return
    /// `0`.
    ///
    /// This method is realtime-safe.
    #[cfg(feature = "resampler")]
    pub fn resampler_output_delay(&self) -> usize {
        self.resampler_output_delay
    }

    /// Discard a certian number of output frames from the buffer. This can be used
    /// to correct for jitter and avoid excessive overflows and reduce the percieved
    /// audible glitchiness.
    ///
    /// This will discard `frames.min(self.available_frames())` frames.
    ///
    /// Returns the number of output frames that were discarded.
    ///
    /// This method is realtime-safe.
    pub fn discard_frames(&mut self, frames: usize) -> usize {
        self.cons.skip(frames * self.num_channels) / self.num_channels
    }

    /// Read from the channel and store the results in the given output buffer.
    ///
    /// * `output` - The output buffer. You can use one of the types in the
    ///   [`audioadapter_buffers::direct`](crate::audioadapter_buffers::direct) module
    ///   to wrap your input data into a type that implements [`Adapter`].
    /// * `output_range` - The range in each output channel to write to. If this is
    ///   `None`, then the entire output buffer will be read.
    /// * `active_channels_mask` - An optional mask that selects which channels in
    ///   `output` to use. Channels marked with `false` will be filled with zeros.
    ///   If `None`, then all of the channels will be active.
    /// * `output_is_already_cleared` - If `true`, then this will skip the step of
    ///   clearing the output buffer in the range `output_range` to zeros.
    ///
    /// This method is realtime-safe.
    ///
    /// # Panics
    /// Panics if:
    /// * The `input_range` is out of bounds for any of the input channels.
    pub fn read(
        &mut self,
        output: &mut dyn AdapterMut<T>,
        output_range: Option<Range<usize>>,
        active_channels_mask: Option<&[bool]>,
        output_is_already_cleared: bool,
    ) -> ReadStatus {
        self.set_output_stream_ready(true);

        self.poll_reset();

        let (output_start, output_frames) = if let Some(range) = output_range {
            (range.start, range.end - range.start)
        } else {
            (0, output.frames())
        };

        if !output_is_already_cleared {
            output.fill_frames_with(output_start, output_frames, &T::zero());
        }

        if !self.input_stream_ready() {
            return ReadStatus::InputNotReady;
        }

        self.waiting_for_input_to_reset = false;

        let (s1, s2) = self.cons.as_slices();

        let s1_frames = s1.len() / self.num_channels;
        let s1_copy_frames = s1_frames.min(output_frames);

        let s1_wrapper = InterleavedSlice::new(s1, self.num_channels, s1_frames).unwrap();

        for ch_i in 0..output.channels().min(self.num_channels) {
            let channel_active = active_channels_mask
                .as_ref()
                .map(|m| m.get(ch_i).copied().unwrap_or(false))
                .unwrap_or(true);

            if channel_active {
                output.copy_from_other_to_channel(
                    &s1_wrapper,
                    ch_i,
                    ch_i,
                    0,
                    output_start,
                    s1_copy_frames,
                );
            }
        }

        let mut filled_frames = s1_copy_frames;

        if output_frames > s1_copy_frames {
            let s2_frames = s2.len() / self.num_channels;
            let s2_copy_frames = s2_frames.min(output_frames - s1_copy_frames);

            let s2_wrapper = InterleavedSlice::new(s2, self.num_channels, s2_frames).unwrap();

            for ch_i in 0..output.channels().min(self.num_channels) {
                let channel_active = active_channels_mask
                    .as_ref()
                    .map(|m| m.get(ch_i).copied().unwrap_or(false))
                    .unwrap_or(true);

                if channel_active {
                    output.copy_from_other_to_channel(
                        &s2_wrapper,
                        ch_i,
                        ch_i,
                        0,
                        output_start + s1_copy_frames,
                        s2_copy_frames,
                    );
                }
            }

            filled_frames += s2_copy_frames;
        }

        self.cons.skip(filled_frames * self.num_channels);

        if filled_frames < output_frames {
            ReadStatus::UnderflowOccurred {
                num_frames_read: filled_frames,
            }
        } else if let Some(num_frames_discarded) = self.autocorrect_overflows() {
            ReadStatus::OverflowCorrected {
                num_frames_discarded,
            }
        } else {
            ReadStatus::Ok
        }
    }

    /// Read from the channel and store the results into the output buffer
    /// in interleaved format.
    ///
    /// * `output` - The output buffer to write to. The output buffer must
    ///   have the same number of channels as this consumer.
    /// * `buffer_out_is_already_cleared` - If `true`, then this will skip the step of
    ///   clearing the output buffer in the range `output_range` to zeros.
    ///
    /// This method is realtime-safe.
    pub fn read_interleaved(
        &mut self,
        output: &mut [T],
        output_already_cleared: bool,
    ) -> ReadStatus {
        self.set_output_stream_ready(true);

        self.poll_reset();

        if !self.input_stream_ready() {
            if !output_already_cleared {
                output.fill(T::zero());
            }

            return ReadStatus::InputNotReady;
        }

        self.waiting_for_input_to_reset = false;

        let out_frames = output.len() / self.num_channels;
        let out_len = out_frames * self.num_channels;

        let pushed_samples = self.cons.pop_slice(&mut output[..out_len]);

        if pushed_samples < out_len {
            if !output_already_cleared {
                output[pushed_samples..].fill(T::zero());
            }

            ReadStatus::UnderflowOccurred {
                num_frames_read: pushed_samples / self.num_channels,
            }
        } else if let Some(num_frames_discarded) = self.autocorrect_overflows() {
            ReadStatus::OverflowCorrected {
                num_frames_discarded,
            }
        } else {
            ReadStatus::Ok
        }
    }

    /// Poll the channel to see if it got a command to reset.
    ///
    /// Returns `true` if the channel was reset.
    pub fn poll_reset(&mut self) -> bool {
        if self.shared_state.reset.load(Ordering::Relaxed) {
            self.shared_state.reset.store(false, Ordering::Relaxed);
            self.waiting_for_input_to_reset = true;

            self.cons.clear();

            true
        } else {
            false
        }
    }

    /// Manually notify the input stream that the output stream is ready/not ready
    /// to read samples from the channel.
    ///
    /// If this consumer end is being used in a non-realtime context, then it is
    /// a good idea to set this to `true` so that the producer end can start
    /// pushing samples to the channel immediately.
    ///
    /// Note, calling [`ResamplingCons::read`] and
    /// [`ResamplingCons::read_interleaved`] automatically sets the output stream as
    /// ready.
    ///
    /// This method is realtime-safe.
    pub fn set_output_stream_ready(&mut self, ready: bool) {
        self.shared_state
            .output_stream_ready
            .store(ready, Ordering::Relaxed);
    }

    /// Whether or not the input stream is ready to push samples to the channel.
    ///
    /// This method is realtime-safe.
    pub fn input_stream_ready(&self) -> bool {
        self.shared_state.input_stream_ready.load(Ordering::Relaxed)
            && !(self.waiting_for_input_to_reset && self.cons.is_empty())
    }

    /// Correct for any overflows.
    ///
    /// This returns the number of frames (samples in a single channel of audio) that were
    /// discarded due to an overflow occurring. If no overflow occured, then `None`
    /// is returned.
    ///
    /// Note, this method is already automatically called in [`ResamplingCons::read`] and
    /// [`ResamplingCons::read_interleaved`].
    ///
    /// This will have no effect if [`ResamplingChannelConfig::overflow_autocorrect_percent_threshold`]
    /// was set to `None`.
    ///
    /// This method is realtime-safe.
    pub fn autocorrect_overflows(&mut self) -> Option<usize> {
        if let Some(overflow_autocorrect_threshold_samples) =
            self.overflow_autocorrect_threshold_samples
        {
            let len = self.cons.occupied_len();

            if len >= overflow_autocorrect_threshold_samples && len > self.channel_latency_samples {
                let correction_frames = (len - self.channel_latency_samples) / self.num_channels;

                self.discard_frames(correction_frames);

                return Some(correction_frames);
            }
        }

        None
    }
}

/// The status of pushing samples to [`ResamplingProd::push`] and
/// [`ResamplingProd::push_interleaved`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushStatus {
    /// All samples were successfully pushed to the channel.
    Ok,
    /// The output stream is not yet ready to read samples from the channel.
    ///
    /// Note, this can also happen when the channel is reset.
    ///
    /// No samples were pushed to the channel.
    OutputNotReady,
    /// An overflow occured due to the input stream running faster than the
    /// output stream. Some or all of the samples were not pushed to the channel.
    ///
    /// If this occurs, then it may mean that [`ResamplingChannelConfig::capacity_seconds`]
    /// is too low and should be increased.
    OverflowOccurred {
        /// The number of frames (samples in a single channel of audio) that were
        /// successfully pushed to the channel.
        num_frames_pushed: usize,
    },
    /// An underflow occured due to the output stream running faster than the
    /// input stream.
    ///
    /// All of the samples were successfully pushed to the channel, however extra
    /// zero samples were also pushed to the channel to correct for the jitter.
    ///
    /// If this occurs, then it may mean that [`ResamplingChannelConfig::latency_seconds`]
    /// is too low and should be increased.
    UnderflowCorrected {
        /// The number of zero frames that were pushed after the other samples
        /// were pushed.
        num_zero_frames_pushed: usize,
    },
}

/// The status of reading data from [`ResamplingCons::read`] and
/// [`ResamplingCons::read_interleaved`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadStatus {
    /// The output buffer was fully filled with samples from the channel.
    Ok,
    /// The input stream is not yet ready to push samples to the channel.
    ///
    /// Note, this can also happen when the channel is reset.
    ///
    /// The output buffer was filled with zeros.
    InputNotReady,
    /// An underflow occured due to the output stream running faster than the input
    /// stream. Some or all of the samples in the output buffer have been filled with
    /// zeros on the end. This may result in audible audio glitches.
    ///
    /// If this occurs, then it may mean that [`ResamplingChannelConfig::latency_seconds`]
    /// is too low and should be increased.
    UnderflowOccurred {
        /// The number of frames (samples in a single channel of audio) that were
        /// successfully read from the channel. All frames past this have been filled
        /// with zeros.
        num_frames_read: usize,
    },
    /// An overflow occured due to the input stream running faster than the output
    /// stream
    ///
    /// All of the samples in the output buffer were successfully filled with samples,
    /// however a number of frames have also been discarded to correct for the jitter.
    ///
    /// If this occurs, then it may mean that [`ResamplingChannelConfig::capacity_seconds`]
    /// is too low and should be increased.
    OverflowCorrected {
        /// The number of frames that were discarded from the channel (after the
        /// frames have been read into the output buffer).
        num_frames_discarded: usize,
    },
}

struct SharedState {
    reset: AtomicBool,
    input_stream_ready: AtomicBool,
    output_stream_ready: AtomicBool,
}

impl SharedState {
    fn new() -> Self {
        Self {
            reset: AtomicBool::new(false),
            input_stream_ready: AtomicBool::new(false),
            output_stream_ready: AtomicBool::new(false),
        }
    }
}

/*
/// Needed to get around lifetime nonsense
struct AdapterWrapper<'a, 'b, T: Sample> {
    inner: &'a dyn Adapter<'b, T>,
}

// # Safety: This simply wraps each of the trait methods.
unsafe impl<'a, T: Sample + 'static> Adapter<'a, T> for AdapterWrapper<'_, '_, T> {
    /// Safety: This is just wrapping the inner method
    unsafe fn read_sample_unchecked(&self, channel: usize, frame: usize) -> T {
        self.inner.read_sample_unchecked(channel, frame)
    }

    fn read_sample(&self, channel: usize, frame: usize) -> Option<T> {
        self.inner.read_sample(channel, frame)
    }

    fn channels(&self) -> usize {
        self.inner.channels()
    }

    fn frames(&self) -> usize {
        self.inner.frames()
    }

    fn copy_from_channel_to_slice(&self, channel: usize, skip: usize, slice: &mut [T]) -> usize {
        self.inner.copy_from_channel_to_slice(channel, skip, slice)
    }

    fn copy_from_frame_to_slice(&self, frame: usize, skip: usize, slice: &mut [T]) -> usize {
        self.inner.copy_from_frame_to_slice(frame, skip, slice)
    }
}
*/
