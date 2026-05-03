# Audio Visualization CLI - MiruZen (Mizen)

A high-performance command-line tool written in Rust to generate transparent video visualizations from audio files. 

The tool uses `tiny-skia` for blazing-fast software rendering and `rayon` to parallelize frame generation across CPU cores. It pipes the rendered frames directly to `ffmpeg` to generate video files with full alpha transparency support (such as ProRes MOV or WebM Alpha), making the resulting visualizations perfect for dropping into your video editing software of choice.

## Prerequisites

- **Rust** (Cargo) must be installed to compile the tool.
- **FFmpeg** must be installed on your system and available in your `PATH`.

## Building

Clone the repository and build the tool in release mode for optimal performance:

```bash
cargo build --release
```

The compiled binary will be located at `target/release/audio-vis-tool`.

## Usage

```bash
cargo run --release -- [OPTIONS] --input <INPUT> --output <OUTPUT>
```

### Arguments

| Short | Long | Description | Default |
|---|---|---|---|
| `-i` | `--input <FILE>` | Path to the input audio file (supports MP3, WAV, FLAC, etc.) | **Required** |
| `-o` | `--output <FILE>` | Path to the output video file. Use `.mov` for ProRes 4444 or `.webm` for VP9. Both support alpha transparency. | **Required** |
| `-s` | `--style <STYLE>` | The visualization style to render. Available options: `waveform`, `bars`, `circle`. | `waveform` |
| | `--width <PIXELS>` | The width of the output video in pixels. | `1920` |
| | `--height <PIXELS>` | The height of the output video in pixels. | `1080` |
| | `--fps <FPS>` | The framerate of the output video. | `60` |

## Examples

**1. Basic Waveform (WebM)**
Generate a 1080p 60fps waveform as a transparent WebM video.
```bash
cargo run --release -- -i track.mp3 -o visualization.webm
```

**2. Circular Visualizer (4K ProRes MOV)**
Generate a circular visualizer at 4K resolution using the ProRes 4444 codec for use in Final Cut Pro or Premiere.
```bash
cargo run --release -- -i track.wav -o visualization.mov --style circle --width 3840 --height 2160
```

**3. Frequency Bars at 30fps**
Generate a classic frequency spectrum bar visualizer at 30 frames per second.
```bash
cargo run --release -- -i beat.flac -o spectrum.webm --style bars --fps 30
```
