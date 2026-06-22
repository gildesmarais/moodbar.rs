// Rust guideline compliant 2026-06-22

use crate::bands::SpectralBands;
use crate::bands::SPLIT_OVERLAP_FILL_OPACITY;
use crate::render::util::write_split_rect;
use crate::types::{MoodbarAnalysis, SvgShape};

/// Slight extra width added to adjacent SVG elements to prevent subpixel gaps in browser rendering.
const SVG_SUBPIXEL_OVERLAP: f64 = 0.5;

/// Split margin ratio relative to the total height.
const SPLIT_MARGIN_RATIO: f64 = 0.05;

pub(crate) fn is_split_shape(shape: SvgShape) -> bool {
    matches!(
        shape,
        SvgShape::SplitStacked
            | SvgShape::SplitWaveform
            | SvgShape::SplitLanes
            | SvgShape::SplitCentrifugal
            | SvgShape::SplitOverlapping
    )
}

pub(crate) fn render_split(
    out: &mut String,
    analysis: &MoodbarAnalysis,
    shape: SvgShape,
    width: u32,
    height: u32,
    step: f64,
) {
    match shape {
        SvgShape::SplitStacked => render_split_stacked(out, analysis, height, step),
        SvgShape::SplitWaveform => render_split_waveform(out, analysis, height, step),
        SvgShape::SplitLanes => render_split_lanes(out, analysis, height, step),
        SvgShape::SplitCentrifugal => render_split_centrifugal(out, analysis, height, step),
        SvgShape::SplitOverlapping => render_split_overlapping(out, analysis, height, step),
        _ => {}
    }
    let _ = width;
}

fn render_split_stacked(out: &mut String, analysis: &MoodbarAnalysis, height: u32, step: f64) {
    let h_seg = height as f64 / 3.0;
    let channels = analysis.channel_count.max(1);
    let frame_count = analysis.frames.len() / channels;
    for i in 0..frame_count {
        let x = i as f64 * step;
        let offset = i * channels;
        let frame = &analysis.frames[offset..offset + channels];
        let bands = SpectralBands::from_frame(frame);
        let h_b = h_seg * bands.low;
        let h_g = h_seg * bands.mid;
        let h_t = h_seg * bands.high;

        if h_b > 0.0 {
            let y_b = height as f64 - h_b;
            let _ = write_split_rect(
                out,
                "mb-bass",
                x,
                y_b,
                step + SVG_SUBPIXEL_OVERLAP,
                h_b,
                bands.low,
            );
        }
        if h_g > 0.0 {
            let y_g = height as f64 - h_b - h_g;
            let _ = write_split_rect(
                out,
                "mb-mid",
                x,
                y_g,
                step + SVG_SUBPIXEL_OVERLAP,
                h_g,
                bands.mid,
            );
        }
        if h_t > 0.0 {
            let y_t = height as f64 - h_b - h_g - h_t;
            let _ = write_split_rect(
                out,
                "mb-treble",
                x,
                y_t,
                step + SVG_SUBPIXEL_OVERLAP,
                h_t,
                bands.high,
            );
        }
    }
}

fn render_split_waveform(out: &mut String, analysis: &MoodbarAnalysis, height: u32, step: f64) {
    let mid = height as f64 / 2.0;
    let h_seg = height as f64 / 3.0;
    let channels = analysis.channel_count.max(1);
    let frame_count = analysis.frames.len() / channels;
    for i in 0..frame_count {
        let x = i as f64 * step;
        let offset = i * channels;
        let frame = &analysis.frames[offset..offset + channels];
        let bands = SpectralBands::from_frame(frame);
        let h_b = h_seg * bands.low;
        let h_g = h_seg * bands.mid;
        let h_t = h_seg * bands.high;
        let y_b = mid - h_b / 2.0;

        if h_b > 0.0 {
            let _ = write_split_rect(
                out,
                "mb-bass",
                x,
                y_b,
                step + SVG_SUBPIXEL_OVERLAP,
                h_b,
                bands.low,
            );
        }
        if h_g > 0.0 {
            let h_g_half = h_g / 2.0;
            let _ = write_split_rect(
                out,
                "mb-mid",
                x,
                y_b - h_g_half,
                step + SVG_SUBPIXEL_OVERLAP,
                h_g_half,
                bands.mid,
            );
            let _ = write_split_rect(
                out,
                "mb-mid",
                x,
                y_b + h_b,
                step + SVG_SUBPIXEL_OVERLAP,
                h_g_half,
                bands.mid,
            );
        }
        if h_t > 0.0 {
            let h_g_half = h_g / 2.0;
            let h_t_half = h_t / 2.0;
            let _ = write_split_rect(
                out,
                "mb-treble",
                x,
                y_b - h_g_half - h_t_half,
                step + SVG_SUBPIXEL_OVERLAP,
                h_t_half,
                bands.high,
            );
            let _ = write_split_rect(
                out,
                "mb-treble",
                x,
                y_b + h_b + h_g_half,
                step + SVG_SUBPIXEL_OVERLAP,
                h_t_half,
                bands.high,
            );
        }
    }
}

fn render_split_lanes(out: &mut String, analysis: &MoodbarAnalysis, height: u32, step: f64) {
    let g_val = (height as f64 * SPLIT_MARGIN_RATIO).max(1.0).round();
    let h_lane = (height as f64 - 2.0 * g_val) / 3.0;
    let y_t_bottom = h_lane;
    let y_g_bottom = 2.0 * h_lane + g_val;
    let y_b_bottom = height as f64;

    let channels = analysis.channel_count.max(1);
    let frame_count = analysis.frames.len() / channels;
    for i in 0..frame_count {
        let x = i as f64 * step;
        let offset = i * channels;
        let frame = &analysis.frames[offset..offset + channels];
        let bands = SpectralBands::from_frame(frame);
        let h_b = h_lane * bands.low;
        let h_g = h_lane * bands.mid;
        let h_t = h_lane * bands.high;

        if h_b > 0.0 {
            let _ = write_split_rect(
                out,
                "mb-bass",
                x,
                y_b_bottom - h_b,
                step + SVG_SUBPIXEL_OVERLAP,
                h_b,
                bands.low,
            );
        }
        if h_g > 0.0 {
            let _ = write_split_rect(
                out,
                "mb-mid",
                x,
                y_g_bottom - h_g,
                step + SVG_SUBPIXEL_OVERLAP,
                h_g,
                bands.mid,
            );
        }
        if h_t > 0.0 {
            let _ = write_split_rect(
                out,
                "mb-treble",
                x,
                y_t_bottom - h_t,
                step + SVG_SUBPIXEL_OVERLAP,
                h_t,
                bands.high,
            );
        }
    }
}

fn render_split_centrifugal(out: &mut String, analysis: &MoodbarAnalysis, height: u32, step: f64) {
    let mid = height as f64 / 2.0;
    let h_seg = height as f64 / 3.0;
    let channels = analysis.channel_count.max(1);
    let frame_count = analysis.frames.len() / channels;
    for i in 0..frame_count {
        let x = i as f64 * step;
        let offset = i * channels;
        let frame = &analysis.frames[offset..offset + channels];
        let bands = SpectralBands::from_frame(frame);
        let h_b = h_seg * bands.low;
        let h_g = h_seg * bands.mid;
        let h_t = h_seg * bands.high;
        let y_t = mid - h_t / 2.0;

        if h_t > 0.0 {
            let _ = write_split_rect(
                out,
                "mb-treble",
                x,
                y_t,
                step + SVG_SUBPIXEL_OVERLAP,
                h_t,
                bands.high,
            );
        }
        if h_g > 0.0 {
            let h_g_half = h_g / 2.0;
            let h_t_half = h_t / 2.0;
            let _ = write_split_rect(
                out,
                "mb-mid",
                x,
                mid - h_t_half - h_g_half,
                step + SVG_SUBPIXEL_OVERLAP,
                h_g_half,
                bands.mid,
            );
            let _ = write_split_rect(
                out,
                "mb-mid",
                x,
                mid + h_t_half,
                step + SVG_SUBPIXEL_OVERLAP,
                h_g_half,
                bands.mid,
            );
        }
        if h_b > 0.0 {
            let h_b_half = h_b / 2.0;
            let h_g_half = h_g / 2.0;
            let h_t_half = h_t / 2.0;
            let _ = write_split_rect(
                out,
                "mb-bass",
                x,
                mid - h_t_half - h_g_half - h_b_half,
                step + SVG_SUBPIXEL_OVERLAP,
                h_b_half,
                bands.low,
            );
            let _ = write_split_rect(
                out,
                "mb-bass",
                x,
                mid + h_t_half + h_g_half,
                step + SVG_SUBPIXEL_OVERLAP,
                h_b_half,
                bands.low,
            );
        }
    }
}

fn render_split_overlapping(out: &mut String, analysis: &MoodbarAnalysis, height: u32, step: f64) {
    let channels = analysis.channel_count.max(1);
    let frame_count = analysis.frames.len() / channels;
    for i in 0..frame_count {
        let x = i as f64 * step;
        let offset = i * channels;
        let frame = &analysis.frames[offset..offset + channels];
        let bands = SpectralBands::from_frame(frame);
        let h_b = height as f64 * bands.low;
        let h_g = height as f64 * bands.mid;
        let h_t = height as f64 * bands.high;

        if h_b > 0.0 {
            let _ = write_split_rect(
                out,
                "mb-bass",
                x,
                height as f64 - h_b,
                step + SVG_SUBPIXEL_OVERLAP,
                h_b,
                SPLIT_OVERLAP_FILL_OPACITY,
            );
        }
        if h_g > 0.0 {
            let _ = write_split_rect(
                out,
                "mb-mid",
                x,
                height as f64 - h_g,
                step + SVG_SUBPIXEL_OVERLAP,
                h_g,
                SPLIT_OVERLAP_FILL_OPACITY,
            );
        }
        if h_t > 0.0 {
            let _ = write_split_rect(
                out,
                "mb-treble",
                x,
                height as f64 - h_t,
                step + SVG_SUBPIXEL_OVERLAP,
                h_t,
                SPLIT_OVERLAP_FILL_OPACITY,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AnalysisDiagnostics;

    fn fixture_analysis() -> MoodbarAnalysis {
        MoodbarAnalysis {
            channel_count: 3,
            frames: vec![1.0, 0.5, 0.25, 0.8, 0.6, 0.4, 0.2, 0.9, 0.1],
            colors: vec![],
            diagnostics: AnalysisDiagnostics::default(),
            band_colors: vec![[220, 20, 180], [240, 120, 0], [0, 160, 240]],
        }
    }

    #[test]
    fn split_stacked_uses_style_classes() {
        let mut out = String::new();
        render_split_stacked(&mut out, &fixture_analysis(), 96, 400.0);
        assert!(out.contains("class=\"mb-bass\""));
        assert!(out.contains("class=\"mb-mid\""));
        assert!(out.contains("class=\"mb-treble\""));
        assert!(!out.contains("fill=\"rgb("));
    }

    #[test]
    fn split_stacked_layout_is_deterministic() {
        let analysis = fixture_analysis();
        let mut a = String::new();
        let mut b = String::new();
        render_split_stacked(&mut a, &analysis, 96, 400.0);
        render_split_stacked(&mut b, &analysis, 96, 400.0);
        assert_eq!(a, b);
        assert!(a.matches("class=\"mb-bass\"").count() >= 1);
    }

    #[test]
    fn all_split_variants_emit_rects() {
        let analysis = fixture_analysis();
        for shape in [
            SvgShape::SplitStacked,
            SvgShape::SplitWaveform,
            SvgShape::SplitLanes,
            SvgShape::SplitCentrifugal,
            SvgShape::SplitOverlapping,
        ] {
            let mut out = String::new();
            render_split(&mut out, &analysis, shape, 1200, 96, 400.0);
            assert!(out.contains("<rect"), "shape {shape:?} should emit rects");
        }
    }
}
