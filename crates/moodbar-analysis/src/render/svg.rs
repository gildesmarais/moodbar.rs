use std::fmt::Write as _;

use crate::analyze::{frame_to_rgb, frame_to_svg_rgb, scale_to_u8};
use crate::render::util::{
    gradient_stop_indices, SVG_WAVEFORM_FILL_OPACITY, SVG_WAVEFORM_STROKE_OPACITY,
    SVG_WAVEFORM_STROKE_WIDTH,
};
use crate::types::{MoodbarAnalysis, SvgOptions};

pub(crate) fn write_svg_shell(out: &mut String, options: &SvgOptions) -> (u32, u32, f64) {
    let width = options.width.max(1);
    let height = options.height.max(1);
    let count = 1.0_f64; // placeholder; caller sets step from analysis
    write!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {width} {height}\" width=\"{width}\" height=\"{height}\">"
    )
    .unwrap();
    write!(
        out,
        "<rect x=\"0\" y=\"0\" width=\"{width}\" height=\"{height}\" fill=\"{}\"/>",
        options.background
    )
    .unwrap();
    (width, height, count)
}

pub(crate) fn write_gradient_defs(
    out: &mut String,
    analysis: &MoodbarAnalysis,
    options: &SvgOptions,
    width: u32,
    include_style: bool,
) -> &'static str {
    if include_style {
        out.push_str("<defs>");
        crate::render::util::write_split_style_block(out);
    } else {
        out.push_str("<defs>");
    }
    let gradient_id = "mood-gradient";
    write!(
        out,
        "<linearGradient id=\"{gradient_id}\" x1=\"0\" y1=\"0\" x2=\"{width}\" y2=\"0\" gradientUnits=\"userSpaceOnUse\">"
    )
    .unwrap();
    for i in gradient_stop_indices(analysis.frames.len(), options.max_gradient_stops.max(2)) {
        let frame = &analysis.frames[i];
        let denom = (analysis.frames.len().saturating_sub(1)).max(1) as f64;
        let offset = (i as f64 / denom) * 100.0;
        let (r, g, b) = frame_to_svg_rgb(frame);
        write!(
            out,
            "<stop offset=\"{offset:.3}%\" stop-color=\"rgb({r},{g},{b})\"/>"
        )
        .unwrap();
    }
    out.push_str("</linearGradient></defs>");
    gradient_id
}

pub(crate) fn render_strip(
    out: &mut String,
    analysis: &MoodbarAnalysis,
    _width: u32,
    height: u32,
    step: f64,
) {
    for (i, frame) in analysis.frames.iter().enumerate() {
        let x = i as f64 * step;
        let (r, g, b) = frame_to_rgb(frame);
        write!(
            out,
            "<rect x=\"{x:.6}\" y=\"0\" width=\"{:.6}\" height=\"{height}\" fill=\"rgb({},{},{})\"/>",
            step + 0.5,
            scale_to_u8(r),
            scale_to_u8(g),
            scale_to_u8(b)
        )
        .unwrap();
    }
}

pub(crate) fn render_waveform(
    out: &mut String,
    analysis: &MoodbarAnalysis,
    _width: u32,
    height: u32,
    step: f64,
    gradient_id: &str,
) {
    let mid = height as f64 / 2.0;
    let mut d = String::with_capacity(analysis.frames.len().saturating_mul(32));
    for (i, frame) in analysis.frames.iter().enumerate() {
        let x = i as f64 * step;
        let energy = (frame.iter().sum::<f64>() / frame.len().max(1) as f64).clamp(0.0, 1.0);
        let amp = energy * mid * 0.95;
        let y = mid - amp;
        if i == 0 {
            write!(d, "M {x:.6} {y:.6}").unwrap();
        } else {
            write!(d, " L {x:.6} {y:.6}").unwrap();
        }
    }
    for i in (0..analysis.frames.len()).rev() {
        let x = i as f64 * step;
        let frame = &analysis.frames[i];
        let energy = (frame.iter().sum::<f64>() / frame.len().max(1) as f64).clamp(0.0, 1.0);
        let amp = energy * mid * 0.95;
        let y = mid + amp;
        write!(d, " L {x:.6} {y:.6}").unwrap();
    }
    d.push_str(" Z");
    write!(
        out,
        "<path d=\"{d}\" fill=\"url(#{gradient_id})\" fill-opacity=\"{SVG_WAVEFORM_FILL_OPACITY:.2}\" stroke=\"url(#{gradient_id})\" stroke-opacity=\"{SVG_WAVEFORM_STROKE_OPACITY:.2}\" stroke-width=\"{SVG_WAVEFORM_STROKE_WIDTH:.2}\" vector-effect=\"non-scaling-stroke\" stroke-linecap=\"round\" stroke-linejoin=\"round\" shape-rendering=\"geometricPrecision\"/>"
    )
    .unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AnalysisDiagnostics;

    #[test]
    fn strip_and_waveform_render() {
        let analysis = MoodbarAnalysis {
            channel_count: 3,
            frames: vec![
                vec![1.0, 0.0, 0.0],
                vec![0.0, 1.0, 0.2],
                vec![0.0, 0.1, 1.0],
            ],
            colors: vec![[255, 0, 0], [0, 255, 51], [0, 25, 255]],
            diagnostics: AnalysisDiagnostics::default(),
        };

        let mut strip = String::new();
        write_svg_shell(&mut strip, &SvgOptions::default());
        let _gradient_id =
            write_gradient_defs(&mut strip, &analysis, &SvgOptions::default(), 1200, false);
        render_strip(&mut strip, &analysis, 1200, 96, 400.0);
        strip.push_str("</svg>");
        assert!(strip.contains("<svg"));
        assert!(strip.contains("<rect"));
        assert!(strip.contains("<linearGradient"));

        let mut waveform = String::new();
        write_svg_shell(
            &mut waveform,
            &SvgOptions {
                shape: crate::types::SvgShape::Waveform,
                ..SvgOptions::default()
            },
        );
        let gid = write_gradient_defs(
            &mut waveform,
            &analysis,
            &SvgOptions::default(),
            1200,
            false,
        );
        render_waveform(&mut waveform, &analysis, 1200, 96, 400.0, gid);
        waveform.push_str("</svg>");
        assert!(waveform.contains("<path"));
        assert!(waveform.contains("url(#mood-gradient)"));
    }
}
