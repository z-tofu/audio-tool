use std::sync::Arc;

use rustfft::{Fft, FftPlanner, num_complex::Complex};
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

    let mut hint = Hint::new();
    if let Some(ext) = std::path::Path::new(path).extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
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

    let mut all_samples = Vec::with_capacity(track.codec_params.n_frames.unwrap_or(0) as usize);
    // Reused across packets; only reallocated if a packet is larger than the last
    let mut sample_buf: Option<SampleBuffer<f32>> = None;

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
                let spec = *audio_buf.spec();
                let channels = spec.channels.count();
                if sample_buf.as_ref().is_none_or(|b| b.capacity() < audio_buf.capacity() * channels) {
                    sample_buf = Some(SampleBuffer::<f32>::new(audio_buf.capacity() as u64, spec));
                }
                let buf = sample_buf.as_mut().unwrap();
                buf.copy_interleaved_ref(audio_buf);

                let inv = 1.0 / channels as f32;
                all_samples.extend(buf.samples().chunks_exact(channels).map(|chunk| chunk.iter().sum::<f32>() * inv));
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

/// A forward FFT of fixed size with a precomputed Hann window. The plan is
/// created once and shared across threads; callers keep their own buffers.
pub struct Spectrum {
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
}

/// Per-thread scratch space for [`Spectrum::compute`].
#[derive(Default)]
pub struct SpectrumBuffers {
    buffer: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
    pub magnitudes: Vec<f32>,
}

impl Spectrum {
    pub fn new(size: usize) -> Self {
        let fft = FftPlanner::new().plan_fft_forward(size);
        let denom = (size as f32 - 1.0).max(1.0);
        let window = (0..size)
            .map(|i| 0.5 * (1.0 - (std::f32::consts::TAU * i as f32 / denom).cos()))
            .collect();
        Self { fft, window }
    }

    /// Computes magnitudes of the first half of the spectrum into `bufs.magnitudes`.
    /// `samples` shorter than the FFT size (e.g. at the end of the file) are zero-padded.
    pub fn compute(&self, samples: &[f32], bufs: &mut SpectrumBuffers) {
        let n = self.window.len();
        bufs.magnitudes.clear();
        if samples.is_empty() {
            return;
        }

        bufs.buffer.clear();
        bufs.buffer.extend(
            self.window.iter().enumerate()
                .map(|(i, &w)| Complex::new(samples.get(i).copied().unwrap_or(0.0) * w, 0.0)),
        );
        bufs.scratch.resize(self.fft.get_inplace_scratch_len(), Complex::default());
        self.fft.process_with_scratch(&mut bufs.buffer, &mut bufs.scratch);

        let inv_n = 1.0 / n as f32;
        bufs.magnitudes.extend(bufs.buffer[..n / 2].iter().map(|c| c.norm() * inv_n));
    }
}
