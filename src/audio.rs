use rustfft::{FftPlanner, num_complex::Complex};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub struct AudioData {
    pub sample_rate: u32,
    pub samples: Vec<f32>,
}

pub fn load_audio(path: &str) -> AudioData {
    let file = std::fs::File::open(path).expect("Failed to open audio file");
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let hint = Hint::new();
    let format_opts: FormatOptions = Default::default();
    let metadata_opts: MetadataOptions = Default::default();
    let decoder_opts: DecoderOptions = Default::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &format_opts, &metadata_opts)
        .expect("Failed to probe audio format");

    let mut format = probed.format;
    let track = format.default_track().expect("No default track found");
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
    
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &decoder_opts)
        .expect("Failed to create decoder");

    let track_id = track.id;

    let mut all_samples = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(Error::IoError(_)) => break, // EOF
            Err(Error::ResetRequired) => continue,
            Err(e) => panic!("Error reading packet: {:?}", e),
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(audio_buf) => {
                let mut sample_buf = SampleBuffer::<f32>::new(audio_buf.capacity() as u64, *audio_buf.spec());
                sample_buf.copy_interleaved_ref(audio_buf.clone());
                
                let channels = audio_buf.spec().channels.count();
                let samples = sample_buf.samples();
                
                for chunk in samples.chunks_exact(channels) {
                    let sum: f32 = chunk.iter().sum();
                    all_samples.push(sum / channels as f32);
                }
            }
            Err(Error::IoError(_)) => break,
            Err(Error::DecodeError(_)) => continue,
            Err(e) => panic!("Decode error: {:?}", e),
        }
    }

    AudioData {
        sample_rate,
        samples: all_samples,
    }
}

pub fn compute_fft(samples: &[f32]) -> Vec<f32> {
    if samples.is_empty() {
        return vec![];
    }
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(samples.len());
    
    let mut buffer: Vec<Complex<f32>> = samples
        .iter()
        .enumerate()
        .map(|(i, &val)| {
            let multiplier = 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (samples.len() as f32 - 1.0)).cos());
            Complex::new(val * multiplier, 0.0)
        })
        .collect();

    fft.process(&mut buffer);

    let n = samples.len() as f32;
    buffer.iter().take(buffer.len() / 2).map(|c| c.norm() / n).collect()
}
