// Rust guideline compliant 2026-06-22

use std::fmt::Write as _;

use crate::bands::SpectralBands;
use crate::types::SvgShape;

pub(crate) const SVG_WAVEFORM_FILL_OPACITY: f64 = 0.78;
pub(crate) const SVG_WAVEFORM_STROKE_OPACITY: f64 = 0.95;
pub(crate) const SVG_WAVEFORM_STROKE_WIDTH: f64 = 1.60;
pub(crate) const SVG_ESTIMATED_BYTES_PER_FRAME: usize = 96;
pub(crate) const SVG_SPLIT_ESTIMATED_BYTES_PER_RECT: usize = 72;
pub(crate) const SVG_STYLE_BLOCK_BYTES: usize = 256;

pub(crate) fn svg_capacity(frame_count: usize, shape: SvgShape) -> usize {
    let base = frame_count.saturating_mul(SVG_ESTIMATED_BYTES_PER_FRAME) + 512;
    match shape {
        SvgShape::Strip | SvgShape::Waveform => base,
        SvgShape::SplitStacked => {
            frame_count.saturating_mul(3 * SVG_SPLIT_ESTIMATED_BYTES_PER_RECT)
                + SVG_STYLE_BLOCK_BYTES
                + 512
        }
        SvgShape::SplitWaveform | SvgShape::SplitCentrifugal => {
            frame_count.saturating_mul(6 * SVG_SPLIT_ESTIMATED_BYTES_PER_RECT)
                + SVG_STYLE_BLOCK_BYTES
                + 512
        }
        SvgShape::SplitLanes => {
            frame_count.saturating_mul(3 * SVG_SPLIT_ESTIMATED_BYTES_PER_RECT)
                + SVG_STYLE_BLOCK_BYTES
                + 512
        }
        SvgShape::SplitOverlapping => {
            frame_count.saturating_mul(3 * SVG_SPLIT_ESTIMATED_BYTES_PER_RECT)
                + SVG_STYLE_BLOCK_BYTES
                + 512
        }
    }
}

pub(crate) fn write_split_style_block(out: &mut String, band_colors: &[[u8; 3]]) {
    let bass = band_colors.first().copied().unwrap_or([220, 20, 180]);
    let mid = band_colors.get(1).copied().unwrap_or([240, 120, 0]);
    let treble = band_colors.get(2).copied().unwrap_or([0, 160, 240]);
    write!(
        out,
        "<style>.mb-bass{{fill:rgb({},{},{})}}.mb-mid{{fill:rgb({},{},{})}}.mb-treble{{fill:rgb({},{},{})}}</style>",
        bass[0],
        bass[1],
        bass[2],
        mid[0],
        mid[1],
        mid[2],
        treble[0],
        treble[1],
        treble[2]
    )
    .unwrap();
}

pub(crate) fn write_split_rect(
    out: &mut String,
    class: &str,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    opacity: f64,
) -> std::fmt::Result {
    if height <= 0.0 {
        return Ok(());
    }
    write!(
        out,
        "<rect class=\"{class}\" x=\"{x:.6}\" y=\"{y:.6}\" width=\"{width:.6}\" height=\"{height:.6}\" opacity=\"{opacity:.4}\"/>"
    )?;
    Ok(())
}

pub(crate) fn gradient_stop_indices(frame_count: usize, max_stops: usize) -> Vec<usize> {
    if frame_count == 0 {
        return Vec::new();
    }
    if frame_count <= max_stops {
        return (0..frame_count).collect();
    }

    let mut out = Vec::with_capacity(max_stops);
    let denom = (max_stops - 1) as f64;
    let max_idx = (frame_count - 1) as f64;
    for i in 0..max_stops {
        let idx = ((i as f64 / denom) * max_idx).round() as usize;
        if out.last().copied() != Some(idx) {
            out.push(idx);
        }
    }
    if out.last().copied() != Some(frame_count - 1) {
        out.push(frame_count - 1);
    }
    out
}

pub(crate) fn frame_at_x(x: u32, width: u32, frame_count: usize) -> usize {
    if frame_count == 0 {
        return 0;
    }
    (((x as f64 / width as f64) * frame_count as f64).floor() as usize).min(frame_count - 1)
}

#[cfg(feature = "png")]
pub(crate) fn fill_column_raw(buf: &mut [u8], width: u32, x: u32, y0: u32, y1: u32, rgba: [u8; 4]) {
    if y1 <= y0 {
        return;
    }
    let stride = (width * 4) as usize;
    let col_start = (x as usize) * 4;
    for y in y0..y1 {
        let offset = (y as usize) * stride + col_start;
        buf[offset..offset + 4].copy_from_slice(&rgba);
    }
}

#[cfg(feature = "png")]
pub(crate) fn blend_rgba_over(bg: [u8; 4], fg: [u8; 4]) -> [u8; 4] {
    let alpha_fg = fg[3] as f32 / 255.0;
    let alpha_bg = bg[3] as f32 / 255.0;
    let out_alpha = alpha_fg + alpha_bg * (1.0 - alpha_fg);
    if out_alpha <= 0.0 {
        return [0, 0, 0, 0];
    }
    let blend = |c_fg: u8, c_bg: u8| -> u8 {
        let f = c_fg as f32 / 255.0;
        let b = c_bg as f32 / 255.0;
        let out = (f * alpha_fg + b * alpha_bg * (1.0 - alpha_fg)) / out_alpha;
        (out * 255.0).round() as u8
    };
    [
        blend(fg[0], bg[0]),
        blend(fg[1], bg[1]),
        blend(fg[2], bg[2]),
        (out_alpha * 255.0).round() as u8,
    ]
}

pub(crate) struct ColumnFrameCache<'a> {
    last_idx: Option<usize>,
    bands: SpectralBands,
    frame: Option<&'a [f64]>,
}

impl<'a> ColumnFrameCache<'a> {
    pub(crate) fn new() -> Self {
        Self {
            last_idx: None,
            bands: SpectralBands::from_frame(&[]),
            frame: None,
        }
    }

    pub(crate) fn resolve(
        &mut self,
        idx: usize,
        frames: &'a [f64],
        channel_count: usize,
    ) -> (&'a [f64], SpectralBands) {
        if self.last_idx != Some(idx) {
            let offset = idx * channel_count;
            let frame = if offset + channel_count <= frames.len() {
                &frames[offset..offset + channel_count]
            } else {
                &[]
            };
            self.bands = SpectralBands::from_frame(frame);
            self.frame = Some(frame);
            self.last_idx = Some(idx);
        }
        (self.frame.unwrap_or(&[]), self.bands)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gradient_stop_count_is_capped() {
        let stops = gradient_stop_indices(5000, 256);
        assert!(stops.len() <= 256);
        assert!(stops.len() > 1);
        assert_eq!(*stops.last().unwrap(), 4999);
    }

    #[test]
    fn write_split_rect_skips_zero_height() {
        let mut out = String::new();
        write_split_rect(&mut out, "mb-bass", 0.0, 0.0, 1.0, 0.0, 0.5).unwrap();
        assert!(out.is_empty());
    }
}
