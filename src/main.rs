mod audio;
mod render;
mod export;

use clap::Parser;
use rayon::prelude::*;
use indicatif::ProgressBar;
use render::Style;

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
    #[arg(long, default_value_t = 60)]
    fps: u32,
}

fn main() {
    let args = Args::parse();

    println!("Loading audio from {}...", args.input);
    let audio_data = audio::load_audio(&args.input);
    
    let total_samples = audio_data.samples.len();
    let duration_sec = total_samples as f32 / audio_data.sample_rate as f32;
    let total_frames = (duration_sec * args.fps as f32) as usize;
    let samples_per_frame = audio_data.sample_rate / args.fps;

    println!("Audio loaded: {} Hz, {} sec", audio_data.sample_rate, duration_sec);
    println!("Rendering {} frames at {} fps...", total_frames, args.fps);

    let mut exporter = export::Exporter::new(&args.output, &args.input, args.width, args.height, args.fps);
    let pb = ProgressBar::new(total_frames as u64);

    let batch_size = args.fps as usize; // process 1 second batches
    
    for chunk_start in (0..total_frames).step_by(batch_size) {
        let chunk_end = (chunk_start + batch_size).min(total_frames);
        
        let frames: Vec<_> = (chunk_start..chunk_end).into_par_iter().map(|frame_idx| {
            let start_sample = frame_idx * samples_per_frame as usize;
            // take a larger window for fft to get better resolution
            let end_sample = (start_sample + (samples_per_frame as usize * 4)).min(total_samples); 
            
            let samples = if start_sample < total_samples {
                &audio_data.samples[start_sample..end_sample]
            } else {
                &[]
            };
            
            let fft_data = audio::compute_fft(samples);
            render::render_frame(args.width, args.height, &args.style, samples, &fft_data)
        }).collect();
        
        for frame in frames {
            exporter.write_frame(&frame);
            pb.inc(1);
        }
    }
    
    pb.finish_with_message("Rendering complete");
    exporter.finish();
    println!("Output saved to {}", args.output);
}
