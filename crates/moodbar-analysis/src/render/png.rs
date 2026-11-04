// Rust guideline compliant 2026-06-22

use crate::render::util::{fill_column_raw, frame_at_x};
use crate::types::{MoodbarAnalysis, PngOptions, SvgShape};

/// Waveform vertical scaling factor.
/// Prevents waveform peaks from clipping against the image borders.
const WAVEFORM_SCALE: f64 = 0.95;

/// Alpha transparency value for the main waveform body.
const WAVEFORM_FILL_ALPHA: u8 = 210;

/// Alpha transparency value for the waveform border.
const WAVEFORM_BORDER_ALPHA: u8 = 255;

#[cfg(feature = "png")]
pub(crate) fn render_strip_png(
    buf: &mut [u8],
    analysis: &MoodbarAnalysis,
    width: u32,
    height: u32,
) {
    let len = analysis.frames.len() / analysis.channel_count.max(1);
    for x in 0..width {
        let idx = frame_at_x(x, width, len);
        let rgb = analysis.colors.get(idx).copied().unwrap_or([0, 0, 0]);
        let (r, g, b) = crate::analyze::rgb_to_svg_rgb(rgb);
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
    let channels = analysis.channel_count.max(1);
    let len = analysis.frames.len() / channels;
    for x in 0..width {
        let idx = frame_at_x(x, width, len);
        let offset = idx * channels;
        let frame = &analysis.frames[offset..offset + channels];
        let energy = (frame.iter().sum::<f64>() / channels as f64).clamp(0.0, 1.0);
        let amp = energy * mid * WAVEFORM_SCALE;
        let y_top = (mid - amp).max(0.0).floor() as u32;
        let y_bottom = (mid + amp).min((height - 1) as f64).ceil() as u32;
        let rgb = analysis.colors.get(idx).copied().unwrap_or([0, 0, 0]);
        let (r, g, b) = crate::analyze::rgb_to_svg_rgb(rgb);
        if y_top <= y_bottom {
            fill_column_raw(
                buf,
                width,
                x,
                y_top,
                y_bottom + 1,
                [r, g, b, WAVEFORM_FILL_ALPHA],
            );
            if y_top > 0 {
                fill_column_raw(
                    buf,
                    width,
                    x,
                    y_top - 1,
                    y_top,
                    [r, g, b, WAVEFORM_BORDER_ALPHA],
                );
            }
            if y_bottom + 1 < height {
                fill_column_raw(
                    buf,
                    width,
                    x,
                    y_bottom + 1,
                    y_bottom + 2,
                    [r, g, b, WAVEFORM_BORDER_ALPHA],
                );
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
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out).write_image(
        buf,
        width,
        height,
        image::ColorType::Rgba8.into(),
    )?;
    Ok(out)
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

/// Largest accepted PNG side in pixels.
#[cfg(feature = "png")]
pub(crate) const MAX_PNG_DIMENSION: u32 = 16_384;

/// Largest accepted PNG area: 64 Mpx bounds the RGBA buffer to 256 MiB, which mobile survives.
#[cfg(feature = "png")]
pub(crate) const MAX_PNG_PIXELS: u32 = 64 * 1024 * 1024;

#[cfg(feature = "png")]
pub(crate) fn new_rgba_buffer(
    width: u32,
    height: u32,
) -> Result<Vec<u8>, crate::types::MoodbarError> {
    // Both sides <= 2^14 keep the product within u32, so the area check cannot overflow.
    if width > MAX_PNG_DIMENSION || height > MAX_PNG_DIMENSION || width * height > MAX_PNG_PIXELS {
        return Err(crate::types::MoodbarError::InvalidOptions(format!(
            "png dimensions {width}x{height} exceed {MAX_PNG_DIMENSION}px per side or {MAX_PNG_PIXELS} pixels"
        )));
    }
    Ok(vec![0u8; width as usize * height as usize * 4])
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
            frames: vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            colors: vec![[255, 0, 0], [0, 255, 0], [0, 0, 255]],
            diagnostics: AnalysisDiagnostics::default(),
            band_colors: vec![[255, 0, 0], [0, 255, 0], [0, 0, 255]],
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
            let mut buf = new_rgba_buffer(width, height).expect("buffer");
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

    #[test]
    fn oversized_png_is_rejected_without_allocating() {
        let analysis = MoodbarAnalysis {
            channel_count: 3,
            frames: vec![1.0, 0.0, 0.0],
            colors: vec![[255, 0, 0]],
            diagnostics: AnalysisDiagnostics::default(),
            band_colors: vec![[255, 0, 0], [0, 255, 0], [0, 0, 255]],
        };
        for (width, height) in [
            (65_536, 16_384),
            (MAX_PNG_DIMENSION + 1, 1),
            (1, u32::MAX),
            (MAX_PNG_DIMENSION, MAX_PNG_DIMENSION),
            (MAX_PNG_DIMENSION, MAX_PNG_PIXELS / MAX_PNG_DIMENSION + 1),
        ] {
            let options = PngOptions {
                width,
                height,
                shape: SvgShape::Strip,
            };
            assert!(matches!(
                crate::render_png(&analysis, &options),
                Err(crate::types::MoodbarError::InvalidOptions(_))
            ));
        }
        assert!(new_rgba_buffer(MAX_PNG_DIMENSION, MAX_PNG_PIXELS / MAX_PNG_DIMENSION).is_ok());
        for shape in [SvgShape::Waveform, SvgShape::SplitWaveform] {
            let zero_height = PngOptions {
                width: 8,
                height: 0,
                shape,
            };
            assert!(crate::render_png(&analysis, &zero_height).is_ok());
        }
    }
}
