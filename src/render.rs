use tiny_skia::*;

#[derive(Clone, clap::ValueEnum, Debug)]
pub enum Style {
    Waveform,
    Bars,
    Circle,
}

pub fn render_frame(
    width: u32,
    height: u32,
    style: &Style,
    samples: &[f32],
    fft_data: &[f32],
) -> Vec<u8> {
    let mut pixmap = Pixmap::new(width, height).unwrap();
    
    // Clear with transparent background
    pixmap.fill(Color::TRANSPARENT);
    
    let mut paint = Paint::default();
    paint.set_color_rgba8(255, 255, 255, 255);
    paint.anti_alias = true;

    match style {
        Style::Waveform => {
            if samples.is_empty() { return pixmap.data().to_vec(); }
            
            let mut path_builder = PathBuilder::new();
            let step_x = width as f32 / samples.len() as f32;
            let mid_y = height as f32 / 2.0;
            
            path_builder.move_to(0.0, mid_y);
            
            for (i, &sample) in samples.iter().enumerate() {
                let x = i as f32 * step_x;
                let y = mid_y - (sample * mid_y * 0.8);
                path_builder.line_to(x, y);
            }
            
            if let Some(path) = path_builder.finish() {
                let mut stroke = Stroke::default();
                stroke.width = 3.0;
                pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            }
        }
        Style::Bars => {
            if fft_data.is_empty() { return pixmap.data().to_vec(); }
            // Use only the lower half of frequencies which contain most of the energy
            let num_bars = (fft_data.len() / 4).min(200).max(1);
            let bar_width = width as f32 / num_bars as f32;
            
            for i in 0..num_bars {
                // simple log scaling to boost visual impact
                let mag = (fft_data[i] * 1000.0).log10().max(0.0) / 4.0;
                let h = (mag * height as f32).min(height as f32 * 0.9);
                let x = i as f32 * bar_width;
                let y = height as f32 - h;
                
                let rect = Rect::from_xywh(x + 1.0, y, (bar_width - 2.0).max(1.0), h).unwrap();
                pixmap.fill_rect(rect, &paint, Transform::identity(), None);
            }
        }
        Style::Circle => {
            if fft_data.is_empty() { return pixmap.data().to_vec(); }
            let num_points = (fft_data.len() / 4).min(200).max(1);
            let mid_x = width as f32 / 2.0;
            let mid_y = height as f32 / 2.0;
            let base_radius = (width.min(height) as f32) * 0.2;
            
            let mut path_builder = PathBuilder::new();
            
            for i in 0..num_points {
                let mag = (fft_data[i] * 1000.0).log10().max(0.0) / 4.0;
                let radius = base_radius + (mag * base_radius);
                let angle = (i as f32 / num_points as f32) * std::f32::consts::PI * 2.0;
                
                let x = mid_x + angle.cos() * radius;
                let y = mid_y + angle.sin() * radius;
                
                if i == 0 {
                    path_builder.move_to(x, y);
                } else {
                    path_builder.line_to(x, y);
                }
            }
            path_builder.close();
            
            if let Some(path) = path_builder.finish() {
                let mut stroke = Stroke::default();
                stroke.width = 4.0;
                pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            }
        }
    }

    pixmap.data().to_vec()
}
