// This example loads an audio file and plays it through the system's audio
// output, seamlessly looping it 4 times.

const LOOP_COUNT: usize = 4;

use cpal::{
    SupportedBufferSize,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use std::{
    num::NonZeroU32,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use symphonium::{DecodeConfig, DecodedAudio, cache::SymphoniumCache};

pub fn main() {
    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_max_level(tracing::Level::DEBUG)
            .finish(),
    )
    .unwrap();

    let args: Vec<String> = std::env::args().collect();
    if args.len() == 1 {
        println!(
            "usage: cargo run --example clean_loop <path-to-audio-file>\ne.g. cargo run --example clean_loop test_files/ambient_loop_44100.ogg"
        );
        return;
    }
    let mut file_path = std::env::current_dir().unwrap();
    file_path.push(&args[1]);

    let host = cpal::default_host();
    let device = host.default_output_device().unwrap();
    let config = device.default_output_config().unwrap();

    let target_sample_rate = NonZeroU32::new(config.sample_rate()).unwrap();
    let channels = config.channels() as usize;
    let max_buffer_size = match config.buffer_size() {
        SupportedBufferSize::Range { max, .. } => *max,
        SupportedBufferSize::Unknown => 8192,
    } as usize;
    assert!(channels == 2);

    tracing::info!("Selected stream sample rate: {}", target_sample_rate);

    // -----------------------------------------------------------------------

    // An optional cache to re-use decoders and resamplers.
    let cache = SymphoniumCache::default();

    // Probe the audio file.
    let probed = symphonium::probe_from_file(
        file_path,
        // A custom codec prober. Set to `None` to use the default one from symphonia.
        None,
    )
    .unwrap();

    // Decode the probed data.
    let audio_data = symphonium::decode(
        probed,
        &DecodeConfig {
            gapless: true,
            ..Default::default()
        },
        // Set to `None` to keep the original sample rate of the file.
        Some(target_sample_rate),
        // Set to `None` if no cache is needed.
        Some(&cache),
        // A custom codec registry. Set to `None` to use the default one from symphonia.
        None,
    )
    .unwrap();

    dbg!(&audio_data);

    // -----------------------------------------------------------------------

    let mut playhead = 0;

    let loop_count = Arc::new(AtomicUsize::new(0));
    let loop_count_1 = Arc::clone(&loop_count);
    let loop_count_2 = Arc::clone(&loop_count);

    let mut temp_buf_l = vec![0.0; max_buffer_size];
    let mut temp_buf_r = vec![0.0; max_buffer_size];

    let stream = device
        .build_output_stream(
            config.config(),
            move |output: &mut [f32], _: &cpal::OutputCallbackInfo| {
                process(
                    output,
                    &audio_data,
                    &mut playhead,
                    &mut temp_buf_l,
                    &mut temp_buf_r,
                    &loop_count_1,
                )
            },
            move |e| {
                tracing::error!("an error occured on stream: {}", e);
                loop_count_2.store(LOOP_COUNT, Ordering::Relaxed);
            },
            None,
        )
        .unwrap();
    stream.play().unwrap();

    while loop_count.load(Ordering::Relaxed) < LOOP_COUNT {
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn process(
    output: &mut [f32],
    audio_data: &DecodedAudio,
    playhead: &mut usize,
    temp_buf_l: &mut [f32],
    temp_buf_r: &mut [f32],
    loop_count: &Arc<AtomicUsize>,
) {
    let frames = output.len() / 2;

    let first_copy_frames = if *playhead + frames > audio_data.frames() {
        audio_data.frames() - *playhead
    } else {
        frames
    };

    audio_data.fill_stereo(
        *playhead,
        &mut temp_buf_l[..first_copy_frames],
        &mut temp_buf_r[..first_copy_frames],
    );

    *playhead += first_copy_frames;

    if first_copy_frames < frames {
        loop_count.fetch_add(1, Ordering::Relaxed);
        *playhead = 0;

        let second_copy_frames = frames - first_copy_frames;

        audio_data.fill_stereo(
            *playhead,
            &mut temp_buf_l[first_copy_frames..first_copy_frames + second_copy_frames],
            &mut temp_buf_r[first_copy_frames..first_copy_frames + second_copy_frames],
        );

        *playhead += second_copy_frames;
    }

    // Interleave the data into the output.
    for (out, (&in1, &in2)) in output
        .chunks_exact_mut(2)
        .zip(temp_buf_l.iter().zip(temp_buf_r.iter()))
    {
        out[0] = in1;
        out[1] = in2;
    }
}
