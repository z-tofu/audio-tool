mod audio;
mod render;
mod export;

use clap::Parser;
use rayon::prelude::*;
use indicatif::ProgressBar;
use render::{ProgressConfig, ProgressPosition, ProgressStyle, RenderConfig, Rgba, Style, parse_color};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input audio file
    #[arg(short, long)]
    input: String,

    /// Output video file (e.g. out.webm or out.mov)
    #[arg(short, long)]
    output: String,

    /// Visualization style
    #[arg(short, long, value_enum, default_value_t = Style::Waveform)]
    style: Style,

    /// Video width
    #[arg(long, default_value_t = 1920)]
    width: u32,

    /// Video height
    #[arg(long, default_value_t = 1080)]
    height: u32,

    /// Frames per second
    #[arg(long, default_value_t = 60, value_parser = clap::value_parser!(u32).range(1..))]
    fps: u32,

    /// Visualization color (#RRGGBB or #RRGGBBAA)
    #[arg(long, value_parser = parse_color, default_value = "#FFFFFF")]
    color: Rgba,

    /// Background color (#RRGGBB, #RRGGBBAA or "transparent")
    #[arg(long, value_parser = parse_color, default_value = "transparent")]
    background: Rgba,

    /// Line thickness in pixels for the waveform and circle styles
    #[arg(long, default_value_t = 3.0)]
    line_width: f32,

    /// Amplitude multiplier; raise for quiet tracks, lower for loud ones
    #[arg(long, default_value_t = 1.0)]
    gain: f32,

    /// Number of bars (bars style) or points (circle style)
    #[arg(long, default_value_t = 200, value_parser = clap::value_parser!(u32).range(1..))]
    bar_count: u32,

    /// Gap between bars in pixels
    #[arg(long, default_value_t = 2.0)]
    bar_gap: f32,

    /// Base radius of the circle style, as a fraction of the smaller video dimension
    #[arg(long, default_value_t = 0.2)]
    radius: f32,

    /// Progress bar showing the playback position over the length of the input
    #[arg(long, value_enum, default_value_t = ProgressStyle::None)]
    progress: ProgressStyle,

    /// Where to place the progress bar
    #[arg(long, value_enum, default_value_t = ProgressPosition::Bottom)]
    progress_position: ProgressPosition,

    /// Color of the elapsed part of the progress bar [default: same as --color]
    #[arg(long, value_parser = parse_color)]
    progress_color: Option<Rgba>,

    /// Color of the remaining part of the progress bar (bar and dot styles)
    #[arg(long, value_parser = parse_color, default_value = "#FFFFFF40")]
    progress_track_color: Rgba,

    /// Progress bar thickness in pixels
    #[arg(long, default_value_t = 6.0)]
    progress_thickness: f32,

    /// Distance in pixels between the progress bar and the video edges
    #[arg(long, default_value_t = 40.0)]
    progress_margin: f32,
}

fn main() {
    let args = Args::parse();

    println!("Loading audio from {}...", args.input);
    let audio_data = audio::load_audio(&args.input);

    let total_samples = audio_data.samples.len();
    let duration_sec = total_samples as f64 / audio_data.sample_rate as f64;
    let total_frames = (duration_sec * args.fps as f64) as usize;
    // Kept fractional so e.g. 44100 Hz at 24 fps doesn't drift out of sync
    let samples_per_frame = audio_data.sample_rate as f64 / args.fps as f64;
    // take a larger window for fft to get better resolution
    let window_len = (samples_per_frame * 4.0) as usize;
    let fft_len = window_len.next_power_of_two();
    let spectrum = audio::Spectrum::new(fft_len);

    println!("Audio loaded: {} Hz, {:.2} sec", audio_data.sample_rate, duration_sec);
    println!("Rendering {} frames at {} fps...", total_frames, args.fps);

    let cfg = RenderConfig {
        width: args.width,
        height: args.height,
        style: args.style,
        color: args.color,
        background: args.background,
        line_width: args.line_width,
        gain: args.gain,
        bar_count: args.bar_count as usize,
        bar_gap: args.bar_gap,
        radius: args.radius,
        progress: ProgressConfig {
            style: args.progress,
            position: args.progress_position,
            color: args.progress_color.unwrap_or(args.color),
            track_color: args.progress_track_color,
            thickness: args.progress_thickness,
            margin: args.progress_margin,
        },
    };

    let mut exporter = export::Exporter::new(&args.output, &args.input, args.width, args.height, args.fps);
    let pb = ProgressBar::new(total_frames as u64);

    // Small batches bound memory (a 1080p frame is ~8 MB); the writer thread lets
    // ffmpeg consume one batch while the next one renders.
    let batch_size = rayon::current_num_threads() * 2;
    let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<Vec<u8>>>(1);

    std::thread::scope(|scope| {
        let pb = &pb;
        let writer = scope.spawn(move || {
            for frames in rx {
                for frame in frames {
                    exporter.write_frame(&frame);
                    pb.inc(1);
                }
            }
            exporter
        });

        for chunk_start in (0..total_frames).step_by(batch_size) {
            let chunk_end = (chunk_start + batch_size).min(total_frames);

            let frames: Vec<_> = (chunk_start..chunk_end).into_par_iter().map_init(
                audio::SpectrumBuffers::default,
                |bufs, frame_idx| {
                    let start_sample = ((frame_idx as f64 * samples_per_frame).round() as usize).min(total_samples);
                    let window_end = (start_sample + window_len).min(total_samples);
                    let fft_end = (start_sample + fft_len).min(total_samples);

                    spectrum.compute(&audio_data.samples[start_sample..fft_end], bufs);
                    let progress = frame_idx as f32 / (total_frames - 1).max(1) as f32;
                    render::render_frame(&cfg, &audio_data.samples[start_sample..window_end], &bufs.magnitudes, progress)
                },
            ).collect();

            if tx.send(frames).is_err() {
                break; // writer thread panicked; join below reports it
            }
        }
        drop(tx);

        let mut exporter = writer.join().expect("Writer thread failed");
        pb.finish_with_message("Rendering complete");
        exporter.finish();
    });

    println!("Output saved to {}", args.output);
}
