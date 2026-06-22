pub(crate) fn frame_to_rgb(frame: &[f64]) -> (f64, f64, f64) {
    if frame.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    if frame.len() == 1 {
        let v = frame[0];
        return (v, v, v);
    }
    if frame.len() == 2 {
        return (frame[0], frame[1], 0.0);
    }
    if frame.len() == 3 {
        return (frame[0], frame[1], frame[2]);
    }

    // For >3 channels, map dominant channel to hue while preserving total intensity.
    let mut max_idx = 0usize;
    let mut max_val = frame[0];
    let mut sum = 0.0f64;
    for (i, v) in frame.iter().enumerate() {
        sum += *v;
        if *v > max_val {
            max_idx = i;
            max_val = *v;
        }
    }
    let intensity = (sum / frame.len() as f64).clamp(0.0, 1.0);
    let hue = max_idx as f64 / frame.len() as f64;
    hsv_to_rgb(hue, 0.85, intensity)
}

pub(crate) fn rgb_to_svg_rgb(rgb: [u8; 3]) -> (u8, u8, u8) {
    let r = rgb[0] as f64 / 255.0;
    let g = rgb[1] as f64 / 255.0;
    let b = rgb[2] as f64 / 255.0;
    let peak = r.max(g).max(b);
    if peak <= 1e-12 {
        return (0, 0, 0);
    }

    // Keep hue from channel ratios but increase chroma for clearer default SVG rendering.
    let sr = (r / peak).clamp(0.0, 1.0);
    let sg = (g / peak).clamp(0.0, 1.0);
    let sb = (b / peak).clamp(0.0, 1.0);
    let brightness = (0.30 + 0.70 * peak).clamp(0.0, 1.0);

    (
        scale_to_u8(sr * brightness),
        scale_to_u8(sg * brightness),
        scale_to_u8(sb * brightness),
    )
}

pub(crate) fn scale_to_u8(v: f64) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

pub(crate) fn hsv_to_rgb(h: f64, s: f64, v: f64) -> (f64, f64, f64) {
    let h = (h.fract() * 6.0).max(0.0);
    let c = v * s;
    let x = c * (1.0 - ((h % 2.0) - 1.0).abs());
    let (r1, g1, b1) = if h < 1.0 {
        (c, x, 0.0)
    } else if h < 2.0 {
        (x, c, 0.0)
    } else if h < 3.0 {
        (0.0, c, x)
    } else if h < 4.0 {
        (0.0, x, c)
    } else if h < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    let m = v - c;
    (r1 + m, g1 + m, b1 + m)
}

// Rust guideline compliant 2026-02-21
