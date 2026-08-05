// This example exports the decoded samples to a WAV file.

use std::{num::NonZeroU32, path::PathBuf};

use clap::Parser;
use symphonium::DecodeConfig;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// The file to play
    #[arg(short, long)]
    file: PathBuf,

    /// The output sampling rate
    #[arg(short, long)]
    samplerate: u32,
}

pub fn main() {
    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_max_level(tracing::Level::DEBUG)
            .finish(),
    )
    .unwrap();

    let args = Args::parse();

    // -----------------------------------------------------------------------

    // Probe the audio file.
    let probed = symphonium::probe_from_file(
        &args.file,
        // A custom codec prober. Set to `None` to use the default one from symphonia.
        None,
    )
    .unwrap();

    // Decode the probed data.
    let audio_data = symphonium::decode_f32(
        probed,
        &DecodeConfig {
            gapless: true,
            ..Default::default()
        },
        Some(NonZeroU32::new(args.samplerate).unwrap()),
        None,
        None,
    )
    .unwrap();

    dbg!(&audio_data);

    // -----------------------------------------------------------------------

    let file_name = args
        .file
        .file_prefix()
        .unwrap()
        .to_string_lossy()
        .to_owned();
    let out_file = args
        .file
        .with_file_name(format!("{}_{}.wav", file_name, args.samplerate));

    let spec = hound::WavSpec {
        channels: audio_data.channels() as u16,
        sample_rate: args.samplerate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(out_file, spec).unwrap();

    for frame in 0..audio_data.frames() {
        for ch in audio_data.data.iter() {
            writer.write_sample(ch[frame]).unwrap();
        }
    }

    writer.finalize().unwrap();
}
