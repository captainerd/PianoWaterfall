# fixed-resample

[![Documentation](https://docs.rs/fixed-resample/badge.svg)](https://docs.rs/fixed-resample)
[![Crates.io](https://img.shields.io/crates/v/fixed-resample.svg)](https://crates.io/crates/fixed-resample)
[![License](https://img.shields.io/crates/l/fixed-resample.svg)](https://codeberg.org/Meadowlark/fixed-resample/blob/main/LICENSE-APACHE)

An easy to use crate for resampling at a fixed ratio.

It supports resampling in both realtime and in non-realtime applications, and also includes a handy realtime-safe spsc channel type that automatically resamples the input stream to match the output stream when needed.

This crate uses [Rubato](https://github.com/henquist/rubato) internally.

## Dev profile performance

Note, this resampler is *very* slow when compiled without optimizations, which can lead to underflows in the spsc channel. Consider enabling optimization for the `dev` profile in your `Cargo.toml`:

```toml
[profile.dev]
opt-level = 1
```

## Non-realtime example

```rust
const IN_SAMPLE_RATE: u32 = 44100;
const OUT_SAMPLE_RATE: u32 = 48000;
const LEN_SECONDS: f64 = 1.0;

// Generate a sine wave at the input sample rate.
let mut phasor: f32 = 0.0;
let phasor_inc: f32 = 440.0 / IN_SAMPLE_RATE as f32;
let len_samples = (LEN_SECONDS * IN_SAMPLE_RATE as f64).round() as usize;
let in_samples: Vec<f32> = (0..len_samples).map(|_| {
    phasor = (phasor + phasor_inc).fract();
    (phasor * std::f32::consts::TAU).sin() * 0.5
}).collect();

let mut resampler = PacketResampler::<f32, Interleaved<f32>>::new(
    1, // number of channels
    IN_SAMPLE_RATE,
    OUT_SAMPLE_RATE,
    Default::default(), // default config
);

// Allocate a buffer for the resampled data.
let output_frames = resampler.out_alloc_frames(in_samples.len() as u64);
let mut out_samples: Vec<f32> = Vec::with_capacity(output_frames as usize);

// Resample to the output buffer. This method is realtime-safe.
resampler.process(
    // The input buffer. You can use one of the types in
    // `fixed_resample::audioadapter_buffers::::*` to wrap your buffer into a type that
    // implements `Adapter`.
    &InterleavedSlice::new(&in_samples, 1, len_samples).unwrap(),
    None, // input range
    None, // active channels mask
    // This method gets called whenever there is new resampled data.
    |data, _frames| {
        out_samples.extend_from_slice(data);
    },
    // Whether or not this is the last (or only) packet of data that
    // will be resampled. This ensures that any leftover samples in
    // the internal resampler are flushed to the output.
    Some(LastPacketInfo {
        // Let the resampler know that we want an exact number of output
        // frames. Otherwise the resampler may add extra padded zeros
        // to the end.
        desired_output_frames: Some(output_frames as u64),
    }),
    // Trim the padded zeros at the beginning introduced by the internal
    // resampler.
    true, // trim delay
);
```

## CPAL loopback example

```rust
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use fixed_resample::{direct::InterleavedSlice, PushStatus, ReadStatus};

fn main() {
    let host = cpal::default_host();

    let input_device = host.default_input_device().unwrap();
    let output_device = host.default_output_device().unwrap();

    let output_config: cpal::StreamConfig = output_device.default_output_config().unwrap().into();

    // Try selecting an input config that matches the output sample rate to
    // avoid resampling.
    let mut input_config = None;
    for config in input_device.supported_input_configs().unwrap() {
        if let Some(config) = config.try_with_sample_rate(output_config.sample_rate) {
            input_config = Some(config);
            break;
        }
    }
    let input_config: cpal::StreamConfig = input_config
        .unwrap_or_else(|| input_device.default_input_config().unwrap())
        .into();

    let input_channels = input_config.channels as usize;
    let output_channels = output_config.channels as usize;

    let (mut prod, mut cons) = fixed_resample::resampling_channel::<f32>(
        input_channels,
        input_config.sample_rate,
        output_config.sample_rate,
        true, // push_interleave_only
        Default::default(), // configuration
    );

    let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
        prod.push_interleaved(data);
    };

    let output_data_fn = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
        let frames = data.len() / output_channels;

        cons.read(
            &mut InterleavedSlice::new_mut(data, output_channels, frames).unwrap(),
            None,  // output_range
            None,  // active_channels_mask
            false, // output_is_already_cleared
        );
    };

    let input_stream = input_device
        .build_input_stream(input_config, input_data_fn, err_fn, None)
        .unwrap();
    let output_stream = output_device
        .build_output_stream(output_config, output_data_fn, err_fn, None)
        .unwrap();

    // Play the streams.
    input_stream.play().unwrap();
    output_stream.play().unwrap();

    // Run for 10 seconds before closing.
    std::thread::sleep(std::time::Duration::from_secs(10));
}

fn err_fn(err: cpal::StreamError) {
    eprintln!("an error occurred on stream: {}", err);
}
```
