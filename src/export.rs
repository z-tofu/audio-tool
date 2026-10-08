use std::process::{Command, Stdio, Child, ChildStdin};
use std::io::Write;

pub struct Exporter {
    child: Child,
    stdin: Option<ChildStdin>,
}

impl Exporter {
    pub fn new(output_path: &str, input_audio: &str, width: u32, height: u32, fps: u32) -> Self {
        let lower = output_path.to_ascii_lowercase();
        let is_webm = lower.ends_with(".webm");
        let is_mov = lower.ends_with(".mov");

        let mut cmd = Command::new("ffmpeg");
        cmd.args(&[
            "-y", // Overwrite output
            "-hide_banner",
            "-loglevel", "error", // Only print real problems
            "-f", "rawvideo",
            "-vcodec", "rawvideo",
            "-s", &format!("{}x{}", width, height),
            "-pix_fmt", "rgba",
            "-r", &fps.to_string(),
            "-i", "-", // Input 0 (video)
            "-i", input_audio, // Input 1 (audio)
        ]);

        if is_webm {
            cmd.args(&[
                "-c:v", "libvpx-vp9",
                "-pix_fmt", "yuva420p",
                "-auto-alt-ref", "0",
                // libvpx defaults are extremely slow; these keep quality reasonable
                // while using all cores
                "-deadline", "good",
                "-cpu-used", "4",
                "-row-mt", "1",
                "-b:v", "0",
                "-crf", "32",
                "-c:a", "libopus",
            ]);
        } else if is_mov {
            cmd.args(&[
                "-c:v", "prores_ks",
                "-profile:v", "4444",
                "-pix_fmt", "yuva444p10le",
                "-c:a", "aac", // Most common supported audio codec for generic mov
            ]);
        } else {
            // Default generic fallback
            cmd.args(&[
                 "-c:v", "libx264",
                 "-pix_fmt", "yuv420p",
                 "-c:a", "aac",
            ]);
        }

        cmd.args(&[
            "-map", "0:v:0",
            "-map", "1:a:0",
            "-shortest", // Stop encoding when the shortest stream ends (usually video)
            output_path
        ]);

        cmd.stdin(Stdio::piped());
        cmd.stderr(Stdio::inherit());
        cmd.stdout(Stdio::null());

        let mut child = cmd.spawn().expect("Failed to start ffmpeg. Make sure ffmpeg is installed and in your PATH.");
        let stdin = child.stdin.take();

        Self { child, stdin }
    }

    pub fn write_frame(&mut self, frame_data: &[u8]) {
        if let Some(stdin) = self.stdin.as_mut() {
            stdin.write_all(frame_data).expect("Failed to write frame to ffmpeg stdin (did ffmpeg exit early?)");
        }
    }

    pub fn finish(&mut self) {
        // Drop stdin to signal EOF
        self.stdin.take();
        let status = self.child.wait().expect("Failed to wait for ffmpeg");
        if !status.success() {
            panic!("ffmpeg exited with {}", status);
        }
    }
}
