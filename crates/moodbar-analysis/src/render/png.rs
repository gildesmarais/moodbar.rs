#[cfg(feature = "png")]
use image::{ImageBuffer, Rgba};

use crate::analyze::frame_to_svg_rgb;
use crate::render::util::{fill_column_raw, frame_at_x};
use crate::types::{MoodbarAnalysis, PngOptions, SvgShape};

#[cfg(feature = "png")]
pub(crate) fn render_strip_png(
    buf: &mut [u8],
    analysis: &MoodbarAnalysis,
    width: u32,
    height: u32,
) {
    let len = analysis.frames.len();
    for x in 0..width {
        let idx = frame_at_x(x, width, len);
        let (r, g, b) = frame_to_svg_rgb(&analysis.frames[idx]);
        fill_column_raw(buf, width, x, 0, height, [r, g, b, 255]);
    }
}

#[cfg(feature = "png")]
pub(crate) fn render_waveform_png(
    buf: &mut [u8],
    analysis: &MoodbarAnalysis,
    width: u32,
    height: u32,
) {
    let mid = height as f64 / 2.0;
    let len = analysis.frames.len();
    for x in 0..width {
        let idx = frame_at_x(x, width, len);
        let frame = &analysis.frames[idx];
        let energy = (frame.iter().sum::<f64>() / frame.len().max(1) as f64).clamp(0.0, 1.0);
        let amp = energy * mid * 0.95;
        let y_top = (mid - amp).max(0.0).floor() as u32;
        let y_bottom = (mid + amp).min((height - 1) as f64).ceil() as u32;
        let (r, g, b) = frame_to_svg_rgb(frame);
        if y_top <= y_bottom {
            fill_column_raw(buf, width, x, y_top, y_bottom + 1, [r, g, b, 210]);
            if y_top > 0 {
                fill_column_raw(buf, width, x, y_top - 1, y_top, [r, g, b, 255]);
            }
            if y_bottom + 1 < height {
                fill_column_raw(buf, width, x, y_bottom + 1, y_bottom + 2, [r, g, b, 255]);
            }
        }
    }
}

#[cfg(feature = "png")]
pub(crate) fn encode_png(
    buf: &[u8],
    width: u32,
    height: u32,
) -> Result<Vec<u8>, crate::types::MoodbarError> {
    use image::ImageEncoder;
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width, height, buf.to_vec()).expect("valid rgba buffer");
    let mut out = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut out);
    encoder.write_image(img.as_raw(), width, height, image::ColorType::Rgba8.into())?;
    Ok(out)
}

#[cfg(feature = "png")]
pub(crate) fn empty_png(width: u32, height: u32) -> Result<Vec<u8>, crate::types::MoodbarError> {
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(width, height, Rgba([0, 0, 0, 0]));
    encode_png(img.as_raw(), width, height)
}

#[cfg(feature = "png")]
pub(crate) fn render_shape_png(buf: &mut [u8], analysis: &MoodbarAnalysis, options: &PngOptions) {
    match options.shape {
        SvgShape::Strip => render_strip_png(buf, analysis, options.width, options.height),
        SvgShape::Waveform => render_waveform_png(buf, analysis, options.width, options.height),
        shape if crate::render::svg_split::is_split_shape(shape) => {
            crate::render::png_split::render_split_png(buf, analysis, options);
        }
        _ => {}
    }
}

#[cfg(feature = "png")]
pub(crate) fn new_rgba_buffer(width: u32, height: u32) -> Vec<u8> {
    vec![0u8; (width * height * 4) as usize]
}

#[cfg(test)]
#[cfg(feature = "png")]
mod tests {
    use super::*;
    use crate::types::AnalysisDiagnostics;

    #[test]
    fn png_render_produces_valid_png_signature() {
        let analysis = MoodbarAnalysis {
            channel_count: 3,
            frames: vec![
                vec![1.0, 0.0, 0.0],
                vec![0.0, 1.0, 0.0],
                vec![0.0, 0.0, 1.0],
            ],
            colors: vec![[255, 0, 0], [0, 255, 0], [0, 0, 255]],
            diagnostics: AnalysisDiagnostics::default(),
        };

        for shape in [
            SvgShape::Strip,
            SvgShape::Waveform,
            SvgShape::SplitStacked,
            SvgShape::SplitWaveform,
            SvgShape::SplitLanes,
            SvgShape::SplitCentrifugal,
            SvgShape::SplitOverlapping,
        ] {
            let width = 64;
            let height = 24;
            let mut buf = new_rgba_buffer(width, height);
            render_shape_png(
                &mut buf,
                &analysis,
                &PngOptions {
                    width,
                    height,
                    shape,
                },
            );
            let png = encode_png(&buf, width, height).expect("encode png");
            assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"), "shape {shape:?}");
        }
    }
}
