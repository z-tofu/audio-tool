use std::process::{Command, Stdio, Child};
use std::io::Write;

pub struct Exporter {
    child: Child,
    #[allow(dead_code)]
    width: u32,
    #[allow(dead_code)]
    height: u32,
}

impl Exporter {
    pub fn new(output_path: &str, input_audio: &str, width: u32, height: u32, fps: u32) -> Self {
        let is_webm = output_path.ends_with(".webm");
        let is_mov = output_path.ends_with(".mov");
        
        let mut cmd = Command::new("ffmpeg");
        cmd.args(&[
            "-y", // Overwrite output
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
        // Uncomment to debug ffmpeg output
        // cmd.stderr(Stdio::inherit());
        cmd.stderr(Stdio::null());
        cmd.stdout(Stdio::null());

        let child = cmd.spawn().expect("Failed to start ffmpeg. Make sure ffmpeg is installed and in your PATH.");

        Self {
            child,
            width,
            height,
        }
    }

    pub fn write_frame(&mut self, frame_data: &[u8]) {
        if let Some(mut stdin) = self.child.stdin.take() {
            stdin.write_all(frame_data).expect("Failed to write frame to ffmpeg stdin");
            self.child.stdin = Some(stdin);
        }
    }

    pub fn finish(&mut self) {
        // Drop stdin to signal EOF
        self.child.stdin.take();
        self.child.wait().expect("ffmpeg process failed");
    }
}
