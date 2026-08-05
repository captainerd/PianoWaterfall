use clap::Parser;
use fixed_resample::{
    audioadapter_buffers::direct::InterleavedSlice, Interleaved, LastPacketInfo, PacketResampler,
    ResampleQuality, ResamplerConfig,
};
use hound::SampleFormat;

#[derive(Parser)]
struct Args {
    /// The path to the audio file to play
    path: std::path::PathBuf,

    /// The target sample rate.
    target_sample_rate: u32,

    /// The resample quality. Valid options are ["very_low", "low", "high", and "high_with_low_latency"].
    quality: String,
}

pub fn main() {
    let args = Args::parse();

    let quality = match args.quality.as_str() {
        "very_low" => ResampleQuality::VeryLow,
        "low" => ResampleQuality::Low,
        "high" => ResampleQuality::High,
        "high_with_low_latency" => ResampleQuality::HighWithLowLatency,
        s => {
            eprintln!("unkown quality type: {}", s);
            println!("Valid options are [\"very_low\", \"low\", \"high\", and \"high_with_low_latency\"]");
            return;
        }
    };

    // --- Load the audio file with hound ----------------------------------------------------

    let mut reader = match hound::WavReader::open(&args.path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to open file: {}", e);
            return;
        }
    };

    let spec = reader.spec();

    if spec.sample_format != SampleFormat::Int || spec.sample_format != SampleFormat::Int {
        eprintln!("Only wav files with 16 bit sample formats are supported in this example.");
        return;
    }

    if spec.sample_rate == args.target_sample_rate {
        println!("Already the same sample rate.");
        return;
    }

    let num_channels = spec.channels as usize;

    let in_samples: Vec<f32> = reader
        .samples::<i16>()
        .map(|s| s.unwrap() as f32 / (std::i16::MAX as f32))
        .collect();

    // --- Resample the contents into the output ---------------------------------------------

    let mut resampler = PacketResampler::<f32, Interleaved<f32>>::new(
        num_channels,
        spec.sample_rate,        // input sample rate
        args.target_sample_rate, // output sample rate
        ResamplerConfig {
            quality,
            ..Default::default()
        },
    );

    // Allocate an output buffer with the needed number of frames.
    let input_frames = in_samples.len() / num_channels;
    let output_frames = resampler.out_alloc_frames(input_frames as u64) as usize;
    let mut out_samples = Vec::new();
    // Since we know we don't need any more samples than this, it's typically a
    // good idea to try and save memory by using `reserve_exact`.
    out_samples.reserve_exact(output_frames * num_channels);

    // Resample to the output buffer. This method is realtime-safe.
    resampler.process(
        // The input buffer. You can use one of the types in `fixed_resample::audioadapter_buffers::::*` to
        // wrap your buffer into a type that implements `Adapter`.
        &InterleavedSlice::new(&in_samples, num_channels, input_frames).unwrap(),
        None, // input range
        None, // active channels mask
        // This method gets called whenever there is a new packet of resampled data.
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

    // --- Write the resampled data to a new wav file ----------------------------------------

    let mut new_file = args.path.clone();
    let file_name = args.path.file_stem().unwrap().to_str().unwrap();
    new_file.set_file_name(format!("{}_res_{}.wav", file_name, args.target_sample_rate));

    let new_spec = hound::WavSpec {
        channels: spec.channels,
        sample_rate: args.target_sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = match hound::WavWriter::create(new_file, new_spec) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Failed to create file: {}", e);
            return;
        }
    };

    for &s in out_samples.iter() {
        writer.write_sample((s * i16::MAX as f32) as i16).unwrap();
    }

    writer.finalize().unwrap();
}
