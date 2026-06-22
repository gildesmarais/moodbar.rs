mod png;
mod png_split;
mod svg;
mod svg_split;
pub(crate) mod util;

use crate::render::svg::{render_strip, render_waveform, write_gradient_defs, write_svg_shell};
use crate::render::svg_split::{is_split_shape, render_split};
use crate::render::util::svg_capacity;
use crate::types::{MoodbarAnalysis, MoodbarError, SvgOptions, SvgShape};

/// Render analyzed frames as SVG output.
pub fn render_svg(analysis: &MoodbarAnalysis, options: &SvgOptions) -> String {
    let width = options.width.max(1);
    let height = options.height.max(1);
    let count = analysis.frames.len().max(1) as f64;
    let step = width as f64 / count;

    let mut s = String::with_capacity(svg_capacity(analysis.frames.len(), options.shape));
    write_svg_shell(&mut s, options);
    let include_style = is_split_shape(options.shape);
    let gradient_id = write_gradient_defs(&mut s, analysis, options, width, include_style);

    match options.shape {
        SvgShape::Strip => render_strip(&mut s, analysis, width, height, step),
        SvgShape::Waveform => render_waveform(&mut s, analysis, width, height, step, gradient_id),
        shape if is_split_shape(shape) => {
            render_split(&mut s, analysis, shape, width, height, step)
        }
        _ => {}
    }

    s.push_str("</svg>");
    s
}

/// Render analyzed frames as PNG bytes.
#[cfg(feature = "png")]
pub fn render_png(
    analysis: &MoodbarAnalysis,
    options: &crate::types::PngOptions,
) -> Result<Vec<u8>, MoodbarError> {
    use crate::render::png::{empty_png, encode_png, new_rgba_buffer, render_shape_png};

    let width = options.width.max(1);
    let height = options.height.max(1);

    if analysis.frames.is_empty() {
        return empty_png(width, height);
    }

    let mut buf = new_rgba_buffer(width, height);
    render_shape_png(&mut buf, analysis, options);
    encode_png(&buf, width, height)
}

#[cfg(test)]
mod bench_tests {
    use super::*;
    use crate::options::GenerateOptions;
    use crate::types::AnalysisDiagnostics;
    use std::time::Instant;

    fn bench_analysis(frame_count: usize) -> MoodbarAnalysis {
        let frames = (0..frame_count)
            .map(|i| {
                let t = i as f64 / frame_count as f64;
                vec![t, 1.0 - t, (0.5 + 0.5 * (t * 10.0).sin()).clamp(0.0, 1.0)]
            })
            .collect();
        MoodbarAnalysis {
            channel_count: 3,
            frames,
            colors: Vec::new(),
            diagnostics: AnalysisDiagnostics::default(),
            band_colors: vec![[255, 0, 0], [0, 255, 0], [0, 0, 255]],
        }
    }

    #[test]
    #[ignore = "dev-only render throughput gate"]
    fn render_bench_1200x96_split_stacked() {
        let analysis = bench_analysis(2000);
        let options = SvgOptions {
            width: 1200,
            height: 96,
            shape: SvgShape::SplitStacked,
            ..SvgOptions::default()
        };

        let start = Instant::now();
        let svg = render_svg(&analysis, &options);
        let svg_elapsed = start.elapsed();

        #[cfg(feature = "png")]
        {
            let png_start = Instant::now();
            let png = render_png(
                &analysis,
                &crate::types::PngOptions {
                    width: 1200,
                    height: 96,
                    shape: SvgShape::SplitStacked,
                },
            )
            .expect("png");
            let png_elapsed = png_start.elapsed();
            eprintln!("PNG render: {:?}, {} bytes", png_elapsed, png.len());
        }

        eprintln!("SVG render: {:?}, {} bytes", svg_elapsed, svg.len());
        assert!(svg.contains("mb-bass"));
        let _ = GenerateOptions::default();
    }
}
