#[cfg(feature = "png")]
use crate::bands::{
    SpectralBands, SPLIT_BAND_HIGH_RGB, SPLIT_BAND_LOW_RGB, SPLIT_BAND_MID_RGB,
    SPLIT_OVERLAP_PNG_ALPHA,
};
use crate::render::util::{blend_rgba_over, fill_column_raw, frame_at_x, ColumnFrameCache};
use crate::types::{MoodbarAnalysis, PngOptions, SvgShape};

#[cfg(feature = "png")]
pub(crate) fn render_split_png(buf: &mut [u8], analysis: &MoodbarAnalysis, options: &PngOptions) {
    match options.shape {
        SvgShape::SplitStacked => {
            render_split_stacked_png(buf, analysis, options.width, options.height)
        }
        SvgShape::SplitWaveform => {
            render_split_waveform_png(buf, analysis, options.width, options.height)
        }
        SvgShape::SplitLanes => {
            render_split_lanes_png(buf, analysis, options.width, options.height)
        }
        SvgShape::SplitCentrifugal => {
            render_split_centrifugal_png(buf, analysis, options.width, options.height)
        }
        SvgShape::SplitOverlapping => {
            render_split_overlapping_png(buf, analysis, options.width, options.height)
        }
        _ => {}
    }
}

#[cfg(feature = "png")]
fn band_rgba(bands: &SpectralBands, band: &str, band_colors: &[[u8; 3]]) -> [u8; 4] {
    let base = match band {
        "low" => band_colors.first().copied().unwrap_or(SPLIT_BAND_LOW_RGB),
        "mid" => band_colors.get(1).copied().unwrap_or(SPLIT_BAND_MID_RGB),
        "high" => band_colors.get(2).copied().unwrap_or(SPLIT_BAND_HIGH_RGB),
        _ => [0, 0, 0],
    };
    let energy = match band {
        "low" => bands.low,
        "mid" => bands.mid,
        "high" => bands.high,
        _ => 0.0,
    };
    let rgb = crate::bands::scale_rgb(base, energy);
    [rgb[0], rgb[1], rgb[2], 255]
}

#[cfg(feature = "png")]
fn render_split_stacked_png(buf: &mut [u8], analysis: &MoodbarAnalysis, width: u32, height: u32) {
    let h_seg = height as f64 / 3.0;
    let len = analysis.frames.len();
    let mut cache = ColumnFrameCache::new();
    for x in 0..width {
        let idx = frame_at_x(x, width, len);
        let (_, bands) = cache.resolve(idx, &analysis.frames);
        let y_bass = (height as f64 - h_seg * bands.low).max(0.0).floor() as u32;
        let y_mid = (y_bass as f64 - h_seg * bands.mid).max(0.0).floor() as u32;
        let y_treble = (y_mid as f64 - h_seg * bands.high).max(0.0).floor() as u32;
        if bands.low > 0.0 {
            fill_column_raw(
                buf,
                width,
                x,
                y_bass,
                height,
                band_rgba(&bands, "low", &analysis.band_colors),
            );
        }
        if bands.mid > 0.0 {
            fill_column_raw(
                buf,
                width,
                x,
                y_mid,
                y_bass,
                band_rgba(&bands, "mid", &analysis.band_colors),
            );
        }
        if bands.high > 0.0 {
            fill_column_raw(
                buf,
                width,
                x,
                y_treble,
                y_mid,
                band_rgba(&bands, "high", &analysis.band_colors),
            );
        }
    }
}

#[cfg(feature = "png")]
fn render_split_waveform_png(buf: &mut [u8], analysis: &MoodbarAnalysis, width: u32, height: u32) {
    let mid = height as f64 / 2.0;
    let h_seg = height as f64 / 3.0;
    let len = analysis.frames.len();
    let mut cache = ColumnFrameCache::new();
    for x in 0..width {
        let idx = frame_at_x(x, width, len);
        let (_, bands) = cache.resolve(idx, &analysis.frames);
        let h_b = h_seg * bands.low;
        let h_g = h_seg * bands.mid;
        let h_t = h_seg * bands.high;
        let y_b_start = (mid - h_b / 2.0).max(0.0).floor() as u32;
        let y_b_end = (mid + h_b / 2.0).min(height as f64).ceil() as u32;
        let h_g_half = h_g / 2.0;
        let h_t_half = h_t / 2.0;
        let y_g_top_start = (mid - h_b / 2.0 - h_g_half).max(0.0).floor() as u32;
        let y_g_bot_end = (mid + h_b / 2.0 + h_g_half).min(height as f64).ceil() as u32;
        let y_t_top_start = (mid - h_b / 2.0 - h_g_half - h_t_half).max(0.0).floor() as u32;
        let y_t_bot_end = (mid + h_b / 2.0 + h_g_half + h_t_half)
            .min(height as f64)
            .ceil() as u32;

        if h_b > 0.0 {
            fill_column_raw(
                buf,
                width,
                x,
                y_b_start,
                y_b_end,
                band_rgba(&bands, "low", &analysis.band_colors),
            );
        }
        if h_g > 0.0 {
            fill_column_raw(
                buf,
                width,
                x,
                y_g_top_start,
                y_b_start,
                band_rgba(&bands, "mid", &analysis.band_colors),
            );
            fill_column_raw(
                buf,
                width,
                x,
                y_b_end,
                y_g_bot_end,
                band_rgba(&bands, "mid", &analysis.band_colors),
            );
        }
        if h_t > 0.0 {
            fill_column_raw(
                buf,
                width,
                x,
                y_t_top_start,
                y_g_top_start,
                band_rgba(&bands, "high", &analysis.band_colors),
            );
            fill_column_raw(
                buf,
                width,
                x,
                y_g_bot_end,
                y_t_bot_end,
                band_rgba(&bands, "high", &analysis.band_colors),
            );
        }
    }
}

#[cfg(feature = "png")]
fn render_split_lanes_png(buf: &mut [u8], analysis: &MoodbarAnalysis, width: u32, height: u32) {
    let g_val = (height as f64 * 0.05).max(1.0).round();
    let h_lane = (height as f64 - 2.0 * g_val) / 3.0;
    let y_t_bottom = h_lane.ceil() as u32;
    let y_g_bottom = (2.0 * h_lane + g_val).ceil() as u32;
    let y_b_bottom = height;
    let len = analysis.frames.len();
    let mut cache = ColumnFrameCache::new();
    for x in 0..width {
        let idx = frame_at_x(x, width, len);
        let (_, bands) = cache.resolve(idx, &analysis.frames);
        let y_b_start = (y_b_bottom as f64 - h_lane * bands.low).max(0.0).floor() as u32;
        let y_g_start = (y_g_bottom as f64 - h_lane * bands.mid).max(0.0).floor() as u32;
        let y_t_start = (y_t_bottom as f64 - h_lane * bands.high).max(0.0).floor() as u32;
        if bands.low > 0.0 {
            fill_column_raw(
                buf,
                width,
                x,
                y_b_start,
                y_b_bottom,
                band_rgba(&bands, "low", &analysis.band_colors),
            );
        }
        if bands.mid > 0.0 {
            fill_column_raw(
                buf,
                width,
                x,
                y_g_start,
                y_g_bottom,
                band_rgba(&bands, "mid", &analysis.band_colors),
            );
        }
        if bands.high > 0.0 {
            fill_column_raw(
                buf,
                width,
                x,
                y_t_start,
                y_t_bottom,
                band_rgba(&bands, "high", &analysis.band_colors),
            );
        }
    }
}

#[cfg(feature = "png")]
fn render_split_centrifugal_png(
    buf: &mut [u8],
    analysis: &MoodbarAnalysis,
    width: u32,
    height: u32,
) {
    let mid = height as f64 / 2.0;
    let h_seg = height as f64 / 3.0;
    let len = analysis.frames.len();
    let mut cache = ColumnFrameCache::new();
    for x in 0..width {
        let idx = frame_at_x(x, width, len);
        let (_, bands) = cache.resolve(idx, &analysis.frames);
        let h_b = h_seg * bands.low;
        let h_g = h_seg * bands.mid;
        let h_t = h_seg * bands.high;
        let y_t_start = (mid - h_t / 2.0).max(0.0).floor() as u32;
        let y_t_end = (mid + h_t / 2.0).min(height as f64).ceil() as u32;
        let h_g_half = h_g / 2.0;
        let h_b_half = h_b / 2.0;
        let h_t_half = h_t / 2.0;
        let y_g_top_start = (mid - h_t_half - h_g_half).max(0.0).floor() as u32;
        let y_g_bot_end = (mid + h_t_half + h_g_half).min(height as f64).ceil() as u32;
        let y_b_top_start = (mid - h_t_half - h_g_half - h_b_half).max(0.0).floor() as u32;
        let y_b_bot_end = (mid + h_t_half + h_g_half + h_b_half)
            .min(height as f64)
            .ceil() as u32;

        if h_t > 0.0 {
            fill_column_raw(
                buf,
                width,
                x,
                y_t_start,
                y_t_end,
                band_rgba(&bands, "high", &analysis.band_colors),
            );
        }
        if h_g > 0.0 {
            fill_column_raw(
                buf,
                width,
                x,
                y_g_top_start,
                y_t_start,
                band_rgba(&bands, "mid", &analysis.band_colors),
            );
            fill_column_raw(
                buf,
                width,
                x,
                y_t_end,
                y_g_bot_end,
                band_rgba(&bands, "mid", &analysis.band_colors),
            );
        }
        if h_b > 0.0 {
            fill_column_raw(
                buf,
                width,
                x,
                y_b_top_start,
                y_g_top_start,
                band_rgba(&bands, "low", &analysis.band_colors),
            );
            fill_column_raw(
                buf,
                width,
                x,
                y_g_bot_end,
                y_b_bot_end,
                band_rgba(&bands, "low", &analysis.band_colors),
            );
        }
    }
}

#[cfg(feature = "png")]
fn render_split_overlapping_png(
    buf: &mut [u8],
    analysis: &MoodbarAnalysis,
    width: u32,
    height: u32,
) {
    let len = analysis.frames.len();
    let mut cache = ColumnFrameCache::new();
    for x in 0..width {
        let idx = frame_at_x(x, width, len);
        let (_, bands) = cache.resolve(idx, &analysis.frames);
        let h_b = height as f64 * bands.low;
        let h_g = height as f64 * bands.mid;
        let h_t = height as f64 * bands.high;
        let y_b_start = (height as f64 - h_b).max(0.0).floor() as u32;
        let y_g_start = (height as f64 - h_g).max(0.0).floor() as u32;
        let y_t_start = (height as f64 - h_t).max(0.0).floor() as u32;

        let y_min = [
            (bands.low > 0.0, y_b_start),
            (bands.mid > 0.0, y_g_start),
            (bands.high > 0.0, y_t_start),
        ]
        .into_iter()
        .filter(|(on, _)| *on)
        .map(|(_, y)| y)
        .min()
        .unwrap_or(height);

        if y_min >= height {
            continue;
        }

        let low_rgba =
            |rgb: [u8; 3]| -> [u8; 4] { [rgb[0], rgb[1], rgb[2], SPLIT_OVERLAP_PNG_ALPHA] };

        let low_base = analysis
            .band_colors
            .first()
            .copied()
            .unwrap_or(SPLIT_BAND_LOW_RGB);
        let mid_base = analysis
            .band_colors
            .get(1)
            .copied()
            .unwrap_or(SPLIT_BAND_MID_RGB);
        let treble_base = analysis
            .band_colors
            .get(2)
            .copied()
            .unwrap_or(SPLIT_BAND_HIGH_RGB);

        for y in y_min..height {
            let mut layers = 0u8;
            let mut current = [0u8; 4];

            if bands.low > 0.0 && y >= y_b_start {
                let fg = low_rgba(crate::bands::scale_rgb(low_base, bands.low));
                current = if layers == 0 {
                    fg
                } else {
                    blend_rgba_over(current, fg)
                };
                layers += 1;
            }
            if bands.mid > 0.0 && y >= y_g_start {
                let fg = low_rgba(crate::bands::scale_rgb(mid_base, bands.mid));
                current = if layers == 0 {
                    fg
                } else {
                    blend_rgba_over(current, fg)
                };
                layers += 1;
            }
            if bands.high > 0.0 && y >= y_t_start {
                let fg = low_rgba(crate::bands::scale_rgb(treble_base, bands.high));
                current = if layers == 0 {
                    fg
                } else {
                    blend_rgba_over(current, fg)
                };
            }

            if current[3] > 0 {
                fill_column_raw(buf, width, x, y, y + 1, current);
            }
        }
    }
}

#[cfg(test)]
#[cfg(feature = "png")]
mod tests {
    use super::*;
    use crate::render::png::new_rgba_buffer;
    use crate::types::AnalysisDiagnostics;

    #[test]
    fn split_stacked_png_layout_is_deterministic() {
        let analysis = MoodbarAnalysis {
            channel_count: 3,
            frames: vec![vec![1.0, 0.5, 0.25], vec![0.8, 0.6, 0.4]],
            colors: vec![],
            diagnostics: AnalysisDiagnostics::default(),
            band_colors: vec![[220, 20, 180], [240, 120, 0], [0, 160, 240]],
        };
        let width = 64;
        let height = 24;
        let mut a = new_rgba_buffer(width, height);
        let mut b = new_rgba_buffer(width, height);
        render_split_stacked_png(&mut a, &analysis, width, height);
        render_split_stacked_png(&mut b, &analysis, width, height);
        assert_eq!(a, b);
    }
}

// Rust guideline compliant 2026-02-21
