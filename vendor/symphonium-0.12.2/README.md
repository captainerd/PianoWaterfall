# Symphonium

[![Documentation](https://docs.rs/symphonium/badge.svg)](https://docs.rs/symphonium)
[![Crates.io](https://img.shields.io/crates/v/symphonium.svg)](https://crates.io/crates/symphonium)
[![License](https://img.shields.io/crates/l/symphonium.svg)](https://codeberg.org/Meadowlark/symphonium/blob/main/LICENSE-MIT)

An unofficial easy-to-use wrapper around [Symphonia](https://github.com/pdeljanov/Symphonia) for loading audio files. It also handles resampling at load-time.

The resulting `DecodedAudio` resources are stored in their native sample format whenever possible to save on memory, and have convenience methods to fill a buffer with `f32` samples from any arbitrary position in the resource in realtime during playback. Alternatively you can use the `DecodedAudioF32` resource if you only need samples in the `f32` format.

## Example

```rust
let target_sample_rate = NonZeroU32::new(44100).unwrap();

// An optional cache to re-use decoders and resamplers.
let cache = SymphoniumCache::default();

// Probe the audio file.
let probed = symphonium::probe_from_file(
    &file_path,
    // A custom codec prober. Set to `None` to use the default one from symphonia.
    None,
)
.unwrap();

// Decode the probed data.
let audio_data = symphonium::decode(
    probed,
    &DecodeConfig::default(),
    // Set to `None` to keep the original sample rate of the file.
    Some(target_sample_rate),
    // Set to `None` if no cache is needed.
    Some(&cache),
    // A custom codec registry. Set to `None` to use the default one from symphonia.
    None,
)
.unwrap();

// Fill a stereo buffer with samples starting at frame 100.
let mut buf_l = vec![0.0f32; 512];
let mut buf_r = vec![0.0f32; 512];
audio_data.fill_stereo(100, &mut buf_l, &mut buf_r);

// Alternatively, if you don't need to save memory, you can
// decode directly to an `f32` format.
let probed = symphonium::probe_from_file(&file_path, None).unwrap();
let audio_data_f32 = symphonium::decode_f32(
        probed,
        &DecodeConfig::default(),
        Some(target_sample_rate),
        Some(&cache),
        None,
    )
    .unwrap();

// Print info about the data (`data` is a `Vec<Vec<f32>>`).
println!("num channels: {}", audio_data_f32.data.len());
println!("num frames: {}", audio_data_f32.data[0].len());
```
## Cargo Features

### Codecs and Container Formats

By default, only `wav`/`pcm` and `ogg`/`vorbis` is enabled. If you need more formats, enable them as features in your `Cargo.toml` file like this:

```toml
symphonium = { version = "0.11", features = ["mp3", "flac"] }
```

Available codecs:

* `aac`
* `adpcm`
* `alac`
* `flac`
* `mp1`
* `mp2`
* `mp3`
* `pcm` (default)
* `vorbis` (default)

Available container formats:

* `caf`
* `isomp4`
* `mkv`
* `ogg` (default)
* `aiff`
* `wav` (default)

Alternatively you can enable these group features:
* `all` - Includes all codecs and container formats
* `all-codecs` - Includes all codecs
* `all-formats` - Includes all container formats
* `open-standards` - Include all royalty-free open standard codecs and formats (adpcm, flac, mkv, ogg, pcm, vorbis, wav)

### Resampling

* `resampler` (default) - Enables resampling at load time
* `fft-resampler` (default) - Enables the fft-based resampling algorithm (recommended)
* `stretch-sinc-resampler` - Enables the "arbitrary sinc" resampler for changing the pitch/length of samples

### Performance

* `opt-simd` (default) - Enable all SIMD support
* `opt-simd-sse` (default)
* `opt-simd-avx` (default)
* `opt-simd-neon` (default)

### Other

* `decode-native` (default) - Enables the `DecodedAudio` type and methods that attempts to decode the file into its native sample format to save on memory. This can be disabled if you only need the `DecodedF32` type.
* `tracing` (default) - Use the `tracing` crate for logging
* `log` - Use the `log` crate for logging

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
