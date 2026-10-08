use tiny_skia::*;

#[derive(Clone, Copy, clap::ValueEnum, Debug)]
pub enum Style {
    Waveform,
    Bars,
    Circle,
}

#[derive(Clone, Copy, clap::ValueEnum, Debug, PartialEq)]
pub enum ProgressStyle {
    /// No progress bar
    None,
    /// Plain filled bar
    Bar,
    /// Bar with a round knob at the playhead (like YouTube)
    Dot,
    /// Bar filling up inside an outlined rectangle
    Boxed,
}

#[derive(Clone, Copy, clap::ValueEnum, Debug)]
pub enum ProgressPosition {
    Top,
    Bottom,
}

/// An RGBA color parsed from `#RRGGBB`, `#RRGGBBAA` or `transparent`.
#[derive(Clone, Copy, Debug)]
pub struct Rgba(pub [u8; 4]);

impl Rgba {
    pub fn is_opaque(&self) -> bool {
        self.0[3] == 255
    }

    fn to_color(self) -> Color {
        let [r, g, b, a] = self.0;
        Color::from_rgba8(r, g, b, a)
    }

    fn paint(self) -> Paint<'static> {
        let [r, g, b, a] = self.0;
        let mut paint = Paint::default();
        paint.set_color_rgba8(r, g, b, a);
        paint.anti_alias = true;
        paint
    }
}

pub fn parse_color(s: &str) -> Result<Rgba, String> {
    if s.eq_ignore_ascii_case("transparent") {
        return Ok(Rgba([0, 0, 0, 0]));
    }
    let hex = s.trim_start_matches('#');
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|e| format!("invalid color '{}': {}", s, e));
    match hex.len() {
        6 => Ok(Rgba([byte(0)?, byte(2)?, byte(4)?, 255])),
        8 => Ok(Rgba([byte(0)?, byte(2)?, byte(4)?, byte(6)?])),
        _ => Err(format!("invalid color '{}': expected #RRGGBB, #RRGGBBAA or 'transparent'", s)),
    }
}

pub struct ProgressConfig {
    pub style: ProgressStyle,
    pub position: ProgressPosition,
    pub color: Rgba,
    pub track_color: Rgba,
    pub thickness: f32,
    pub margin: f32,
}

pub struct RenderConfig {
    pub width: u32,
    pub height: u32,
    pub style: Style,
    pub color: Rgba,
    pub background: Rgba,
    pub line_width: f32,
    pub gain: f32,
    pub bar_count: usize,
    pub bar_gap: f32,
    pub radius: f32,
    pub progress: ProgressConfig,
}

/// Maps an FFT magnitude to 0..~1 with simple log scaling to boost visual impact.
fn scale_magnitude(mag: f32, gain: f32) -> f32 {
    (mag * gain * 1000.0).log10().max(0.0) / 4.0
}

/// Groups the lower quarter of the spectrum (where most of the energy is)
/// into `count` buckets, keeping each bucket's peak.
fn bucket_spectrum(fft_data: &[f32], count: usize) -> impl Iterator<Item = f32> + '_ {
    let bins = (fft_data.len() / 4).max(count).min(fft_data.len());
    (0..count).map(move |i| {
        let start = i * bins / count;
        let end = ((i + 1) * bins / count).max(start + 1);
        fft_data[start..end].iter().copied().fold(0.0, f32::max)
    })
}

/// Renders one frame. `progress` is the playback position in 0..=1.
/// Returns straight (non-premultiplied) RGBA bytes, as ffmpeg expects.
pub fn render_frame(cfg: &RenderConfig, samples: &[f32], fft_data: &[f32], progress: f32) -> Vec<u8> {
    let width = cfg.width as f32;
    let height = cfg.height as f32;
    let mut pixmap = Pixmap::new(cfg.width, cfg.height).unwrap();
    // A fresh pixmap is already zeroed (transparent)
    if cfg.background.0[3] != 0 {
        pixmap.fill(cfg.background.to_color());
    }

    let paint = cfg.color.paint();
    let stroke = Stroke { width: cfg.line_width, ..Stroke::default() };

    match cfg.style {
        Style::Waveform if !samples.is_empty() => {
            let mut path_builder = PathBuilder::new();
            let mid_y = height / 2.0;
            let amp = mid_y * 0.8 * cfg.gain;

            path_builder.move_to(0.0, mid_y);

            if samples.len() <= cfg.width as usize {
                let step_x = width / samples.len() as f32;
                for (i, &sample) in samples.iter().enumerate() {
                    path_builder.line_to(i as f32 * step_x, mid_y - sample * amp);
                }
            } else {
                // More samples than pixels: keep the peak of each column instead of
                // stroking thousands of sub-pixel segments.
                let (n, cols) = (samples.len(), cfg.width as usize);
                for x in 0..cols {
                    let column = &samples[x * n / cols..(x + 1) * n / cols];
                    let peak = column.iter().copied().fold(0.0f32, |p, s| if s.abs() > p.abs() { s } else { p });
                    path_builder.line_to(x as f32, mid_y - peak * amp);
                }
            }

            if let Some(path) = path_builder.finish() {
                pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            }
        }
        Style::Bars if !fft_data.is_empty() => {
            let bar_width = width / cfg.bar_count as f32;

            for (i, mag) in bucket_spectrum(fft_data, cfg.bar_count).enumerate() {
                let h = (scale_magnitude(mag, cfg.gain) * height).min(height * 0.9);
                let x = i as f32 * bar_width;

                if let Some(rect) = Rect::from_xywh(x + cfg.bar_gap / 2.0, height - h, (bar_width - cfg.bar_gap).max(1.0), h) {
                    pixmap.fill_rect(rect, &paint, Transform::identity(), None);
                }
            }
        }
        Style::Circle if !fft_data.is_empty() => {
            let num_points = cfg.bar_count;
            let mid_x = width / 2.0;
            let mid_y = height / 2.0;
            let base_radius = width.min(height) * cfg.radius;

            let mut path_builder = PathBuilder::new();

            for (i, mag) in bucket_spectrum(fft_data, num_points).enumerate() {
                let radius = base_radius + scale_magnitude(mag, cfg.gain) * base_radius;
                let angle = (i as f32 / num_points as f32) * std::f32::consts::TAU;
                let (sin, cos) = angle.sin_cos();
                let (x, y) = (mid_x + cos * radius, mid_y + sin * radius);

                if i == 0 {
                    path_builder.move_to(x, y);
                } else {
                    path_builder.line_to(x, y);
                }
            }
            path_builder.close();

            if let Some(path) = path_builder.finish() {
                pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            }
        }
        _ => {}
    }

    draw_progress(&mut pixmap, &cfg.progress, progress.clamp(0.0, 1.0));

    // tiny-skia stores premultiplied alpha; only translucent pixels need converting
    if cfg.background.is_opaque() {
        pixmap.take()
    } else {
        pixmap.take_demultiplied()
    }
}

fn draw_progress(pixmap: &mut Pixmap, cfg: &ProgressConfig, progress: f32) {
    if cfg.style == ProgressStyle::None {
        return;
    }
    let width = pixmap.width() as f32;
    let height = pixmap.height() as f32;
    let t = cfg.thickness;
    // The dot style needs room for the knob, which is larger than the bar
    let knob_r = t * 1.5;
    let x0 = cfg.margin.max(if cfg.style == ProgressStyle::Dot { knob_r } else { 0.0 });
    let full_w = (width - 2.0 * x0).max(1.0);
    let center_y = match cfg.position {
        ProgressPosition::Top => cfg.margin + t / 2.0,
        ProgressPosition::Bottom => height - cfg.margin - t / 2.0,
    };
    let y0 = center_y - t / 2.0;

    let fill = cfg.color.paint();
    let track = cfg.track_color.paint();
    let id = Transform::identity();

    match cfg.style {
        ProgressStyle::Bar | ProgressStyle::Dot => {
            if let Some(rect) = Rect::from_xywh(x0, y0, full_w, t) {
                pixmap.fill_rect(rect, &track, id, None);
            }
            if let Some(rect) = Rect::from_xywh(x0, y0, full_w * progress, t) {
                pixmap.fill_rect(rect, &fill, id, None);
            }
            if cfg.style == ProgressStyle::Dot
                && let Some(knob) = PathBuilder::from_circle(x0 + full_w * progress, center_y, knob_r)
            {
                pixmap.fill_path(&knob, &fill, FillRule::Winding, id, None);
            }
        }
        ProgressStyle::Boxed => {
            // Outline sits around the bar with a gap equal to the line width
            let line = (t / 4.0).max(1.0);
            let pad = line * 2.0;
            if let Some(outline) = Rect::from_xywh(x0, y0 - pad, full_w, t + 2.0 * pad) {
                let stroke = Stroke { width: line, ..Stroke::default() };
                pixmap.stroke_path(&PathBuilder::from_rect(outline), &fill, &stroke, id, None);
            }
            let inner_w = full_w - 2.0 * pad;
            if let Some(rect) = Rect::from_xywh(x0 + pad, y0, inner_w * progress, t) {
                pixmap.fill_rect(rect, &fill, id, None);
            }
        }
        ProgressStyle::None => {}
    }
}
