// Rust guideline compliant 2026-06-22

#[cfg(feature = "decode")]
use std::path::Path;
use thiserror::Error;

#[cfg(feature = "decode")]
use symphonia::core::errors::Error as SymphoniaError;

/// Visual theme presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    #[default]
    Classic,
    Cool,
    Light,
}

/// Tunable DSP options used by raw and SVG rendering paths.
#[derive(Debug, Clone)]
pub struct GenerateOptions {
    pub fft_size: usize,
    pub low_cut_hz: f32,
    pub mid_cut_hz: f32,
    pub normalize_mode: NormalizeMode,
    pub deterministic_floor: f64,
    pub detection_mode: DetectionMode,
    pub frames_per_color: usize,
    pub band_edges_hz: Vec<f32>,
    pub playback_rate: Option<f32>,
    pub theme: Theme,
    pub custom_colors: Option<Vec<[u8; 3]>>,
}

impl Default for GenerateOptions {
    fn default() -> Self {
        Self {
            fft_size: 2048,
            low_cut_hz: 500.0,
            mid_cut_hz: 2000.0,
            normalize_mode: NormalizeMode::PerChannelPeak,
            deterministic_floor: 1e-12,
            detection_mode: DetectionMode::SpectralEnergy,
            frames_per_color: 1,
            band_edges_hz: vec![500.0, 2000.0],
            playback_rate: None,
            theme: Theme::Classic,
            custom_colors: None,
        }
    }
}

/// Band normalization strategy.
#[derive(Debug, Clone, Copy)]
pub enum NormalizeMode {
    PerChannelPeak,
    GlobalPeak,
}

/// Signal extraction strategy per FFT bin.
#[derive(Debug, Clone, Copy)]
pub enum DetectionMode {
    SpectralEnergy,
    SpectralFlux,
}

/// Non-fatal decoder diagnostics collected during analysis.
#[derive(Debug, Clone, Default)]
pub struct AnalysisDiagnostics {
    pub decode_errors: usize,
    pub zero_channel_packets: usize,
    pub truncated_frames: usize,
}

/// Renderer-agnostic analysis output.
#[derive(Debug, Clone)]
pub struct MoodbarAnalysis {
    pub channel_count: usize,
    pub frames: Vec<f64>,
    pub colors: Vec<[u8; 3]>,
    pub diagnostics: AnalysisDiagnostics,
    pub band_colors: Vec<[u8; 3]>,
}

impl MoodbarAnalysis {
    /// Returns the sequence of colors as a slice of RGB values.
    pub fn colors(&self) -> &[[u8; 3]] {
        &self.colors
    }
}

/// SVG output shape presets.
#[derive(Debug, Clone, Copy)]
pub enum SvgShape {
    Strip,
    Waveform,
    SplitStacked,
    SplitWaveform,
    SplitLanes,
    SplitCentrifugal,
    SplitOverlapping,
}

/// SVG rendering options.
#[derive(Debug, Clone)]
pub struct SvgOptions {
    pub width: u32,
    pub height: u32,
    pub shape: SvgShape,
    pub background: &'static str,
    pub max_gradient_stops: usize,
}

/// PNG rendering options.
#[cfg(feature = "png")]
#[derive(Debug, Clone)]
pub struct PngOptions {
    pub width: u32,
    pub height: u32,
    pub shape: SvgShape,
}

#[cfg(feature = "png")]
impl Default for PngOptions {
    fn default() -> Self {
        Self {
            width: 1200,
            height: 96,
            shape: SvgShape::Strip,
        }
    }
}

impl Default for SvgOptions {
    fn default() -> Self {
        Self {
            width: 1200,
            height: 96,
            shape: SvgShape::Strip,
            background: "transparent",
            max_gradient_stops: 512,
        }
    }
}

/// Errors returned by analysis/decoding APIs.
#[derive(Debug, Error)]
pub enum MoodbarError {
    #[cfg(feature = "decode")]
    #[error("no playable audio track found")]
    NoAudioTrack,
    #[cfg(feature = "decode")]
    #[error("decoded stream has no samples")]
    EmptyAudio,
    #[cfg(feature = "decode")]
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[cfg(feature = "decode")]
    #[error("decode error: {0}")]
    Decode(#[from] SymphoniaError),
    #[cfg(feature = "png")]
    #[error("image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("invalid options: {0}")]
    InvalidOptions(String),
}

fn to_analysis_options(options: &GenerateOptions) -> moodbar_analysis::GenerateOptions {
    moodbar_analysis::GenerateOptions {
        fft_size: options.fft_size,
        low_cut_hz: options.low_cut_hz,
        mid_cut_hz: options.mid_cut_hz,
        normalize_mode: match options.normalize_mode {
            NormalizeMode::PerChannelPeak => moodbar_analysis::NormalizeMode::PerChannelPeak,
            NormalizeMode::GlobalPeak => moodbar_analysis::NormalizeMode::GlobalPeak,
        },
        deterministic_floor: options.deterministic_floor,
        detection_mode: match options.detection_mode {
            DetectionMode::SpectralEnergy => moodbar_analysis::DetectionMode::SpectralEnergy,
            DetectionMode::SpectralFlux => moodbar_analysis::DetectionMode::SpectralFlux,
        },
        frames_per_color: options.frames_per_color,
        band_edges_hz: options.band_edges_hz.clone(),
        max_target_frames: None,
        playback_rate: options.playback_rate,
        theme: match options.theme {
            Theme::Classic => moodbar_analysis::Theme::Classic,
            Theme::Cool => moodbar_analysis::Theme::Cool,
            Theme::Light => moodbar_analysis::Theme::Light,
        },
        custom_colors: options.custom_colors.clone(),
    }
}

fn to_analysis(analysis: &MoodbarAnalysis) -> moodbar_analysis::MoodbarAnalysis {
    moodbar_analysis::MoodbarAnalysis {
        channel_count: analysis.channel_count,
        frames: analysis.frames.clone(),
        colors: analysis.colors.clone(),
        diagnostics: moodbar_analysis::AnalysisDiagnostics {
            decode_errors: analysis.diagnostics.decode_errors,
            zero_channel_packets: analysis.diagnostics.zero_channel_packets,
            truncated_frames: analysis.diagnostics.truncated_frames,
        },
        band_colors: analysis.band_colors.clone(),
    }
}

fn from_analysis(analysis: moodbar_analysis::MoodbarAnalysis) -> MoodbarAnalysis {
    MoodbarAnalysis {
        channel_count: analysis.channel_count,
        frames: analysis.frames,
        colors: analysis.colors,
        diagnostics: AnalysisDiagnostics {
            decode_errors: analysis.diagnostics.decode_errors,
            zero_channel_packets: analysis.diagnostics.zero_channel_packets,
            truncated_frames: analysis.diagnostics.truncated_frames,
        },
        band_colors: analysis.band_colors,
    }
}

fn to_svg_shape(shape: SvgShape) -> moodbar_analysis::SvgShape {
    match shape {
        SvgShape::Strip => moodbar_analysis::SvgShape::Strip,
        SvgShape::Waveform => moodbar_analysis::SvgShape::Waveform,
        SvgShape::SplitStacked => moodbar_analysis::SvgShape::SplitStacked,
        SvgShape::SplitWaveform => moodbar_analysis::SvgShape::SplitWaveform,
        SvgShape::SplitLanes => moodbar_analysis::SvgShape::SplitLanes,
        SvgShape::SplitCentrifugal => moodbar_analysis::SvgShape::SplitCentrifugal,
        SvgShape::SplitOverlapping => moodbar_analysis::SvgShape::SplitOverlapping,
    }
}

fn to_svg_options(options: &SvgOptions) -> moodbar_analysis::SvgOptions {
    moodbar_analysis::SvgOptions {
        width: options.width,
        height: options.height,
        shape: to_svg_shape(options.shape),
        background: options.background,
        max_gradient_stops: options.max_gradient_stops,
    }
}

#[cfg(feature = "png")]
fn to_png_options(options: &PngOptions) -> moodbar_analysis::PngOptions {
    moodbar_analysis::PngOptions {
        width: options.width,
        height: options.height,
        shape: to_svg_shape(options.shape),
    }
}

/// Render analyzed frames as SVG output.
pub fn render_svg(analysis: &MoodbarAnalysis, options: &SvgOptions) -> String {
    moodbar_analysis::render_svg(&to_analysis(analysis), &to_svg_options(options))
}

/// Render analyzed frames as PNG bytes.
#[cfg(feature = "png")]
pub fn render_png(
    analysis: &MoodbarAnalysis,
    options: &PngOptions,
) -> Result<Vec<u8>, MoodbarError> {
    moodbar_analysis::render_png(&to_analysis(analysis), &to_png_options(options)).map_err(|e| {
        match e {
            moodbar_analysis::MoodbarError::InvalidOptions(msg) => {
                MoodbarError::InvalidOptions(msg)
            }
            moodbar_analysis::MoodbarError::Image(err) => MoodbarError::Image(err),
        }
    })
}

/// Convenience API that returns legacy raw RGB bytes.
pub fn analysis_to_raw_rgb_bytes(analysis: &MoodbarAnalysis) -> Vec<u8> {
    let mut out = Vec::<u8>::with_capacity(analysis.colors.len() * 3);
    for color in &analysis.colors {
        out.push(color[0]);
        out.push(color[1]);
        out.push(color[2]);
    }
    out
}

/// Analyze already-decoded mono PCM samples.
pub fn analyze_pcm_mono(
    sample_rate: u32,
    samples: &[f32],
    options: &GenerateOptions,
) -> MoodbarAnalysis {
    let analysis_options = to_analysis_options(options);
    let result = moodbar_analysis::analyze_pcm_mono(sample_rate, samples, &analysis_options);
    from_analysis(result)
}

/// Decode and analyze media into normalized mood frames.
#[cfg(feature = "decode")]
pub fn analyze_path(
    path: &Path,
    options: &GenerateOptions,
) -> Result<MoodbarAnalysis, MoodbarError> {
    let analysis_options = to_analysis_options(options);
    let result = moodbar_decode::analyze_path(path, &analysis_options).map_err(|e| match e {
        moodbar_decode::MoodbarDecodeError::NoAudioTrack => MoodbarError::NoAudioTrack,
        moodbar_decode::MoodbarDecodeError::EmptyAudio => MoodbarError::EmptyAudio,
        moodbar_decode::MoodbarDecodeError::Io(err) => MoodbarError::Io(err),
        moodbar_decode::MoodbarDecodeError::Decode(err) => MoodbarError::Decode(err),
        moodbar_decode::MoodbarDecodeError::InvalidOptions(msg) => {
            MoodbarError::InvalidOptions(msg)
        }
    })?;
    Ok(from_analysis(result))
}

/// Decode and analyze in-memory encoded audio bytes.
#[cfg(feature = "decode")]
pub fn analyze_bytes(
    bytes: &[u8],
    extension: Option<&str>,
    options: &GenerateOptions,
) -> Result<MoodbarAnalysis, MoodbarError> {
    let analysis_options = to_analysis_options(options);
    let result = moodbar_decode::analyze_bytes(bytes, extension, &analysis_options).map_err(
        |e| match e {
            moodbar_decode::MoodbarDecodeError::NoAudioTrack => MoodbarError::NoAudioTrack,
            moodbar_decode::MoodbarDecodeError::EmptyAudio => MoodbarError::EmptyAudio,
            moodbar_decode::MoodbarDecodeError::Io(err) => MoodbarError::Io(err),
            moodbar_decode::MoodbarDecodeError::Decode(err) => MoodbarError::Decode(err),
            moodbar_decode::MoodbarDecodeError::InvalidOptions(msg) => {
                MoodbarError::InvalidOptions(msg)
            }
        },
    )?;
    Ok(from_analysis(result))
}

/// Convenience API that returns legacy raw RGB bytes.
#[cfg(feature = "decode")]
pub fn generate_moodbar_from_path(
    path: &Path,
    options: &GenerateOptions,
) -> Result<Vec<u8>, MoodbarError> {
    let analysis = analyze_path(path, options)?;
    Ok(analysis_to_raw_rgb_bytes(&analysis))
}

/// Convenience API for in-memory encoded audio input.
#[cfg(feature = "decode")]
pub fn generate_moodbar_from_bytes(
    bytes: &[u8],
    extension: Option<&str>,
    options: &GenerateOptions,
) -> Result<Vec<u8>, MoodbarError> {
    let analysis = analyze_bytes(bytes, extension, options)?;
    Ok(analysis_to_raw_rgb_bytes(&analysis))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "decode")]
    #[test]
    fn invalid_band_edges_fail_fast_before_io() {
        let options = GenerateOptions {
            band_edges_hz: vec![2000.0, 500.0],
            ..GenerateOptions::default()
        };
        let res = analyze_path(Path::new("definitely-not-used.wav"), &options);
        assert!(matches!(res, Err(MoodbarError::InvalidOptions(_))));
    }

    #[test]
    fn svg_gradient_stop_count_is_capped() {
        let mut frames = Vec::with_capacity(5000 * 3);
        for i in 0..5000 {
            let t = i as f64 / 5000.0;
            frames.push(t);
            frames.push(1.0 - t);
            frames.push((0.5 + 0.5 * (t * 10.0).sin()).clamp(0.0, 1.0));
        }
        let analysis = MoodbarAnalysis {
            channel_count: 3,
            frames,
            colors: Vec::new(),
            diagnostics: AnalysisDiagnostics::default(),
            band_colors: vec![[255, 0, 0], [0, 255, 0], [0, 0, 255]],
        };
        let svg = render_svg(
            &analysis,
            &SvgOptions {
                max_gradient_stops: 256,
                ..SvgOptions::default()
            },
        );
        let stop_count = svg.matches("<stop ").count();
        assert!(stop_count <= 256);
        assert!(stop_count > 1);
    }

    #[cfg(feature = "png")]
    #[test]
    fn png_render_produces_valid_png_signature() {
        let analysis = MoodbarAnalysis {
            channel_count: 3,
            frames: vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            colors: vec![[255, 0, 0], [0, 255, 0], [0, 0, 255]],
            diagnostics: AnalysisDiagnostics::default(),
            band_colors: vec![[255, 0, 0], [0, 255, 0], [0, 0, 255]],
        };
        let png = render_png(
            &analysis,
            &PngOptions {
                width: 64,
                height: 24,
                shape: SvgShape::Waveform,
            },
        )
        .expect("render png");
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));

        let png_split = render_png(
            &analysis,
            &PngOptions {
                width: 64,
                height: 24,
                shape: SvgShape::SplitStacked,
            },
        )
        .expect("render png split");
        assert!(png_split.starts_with(b"\x89PNG\r\n\x1a\n"));

        for shape in &[
            SvgShape::SplitWaveform,
            SvgShape::SplitLanes,
            SvgShape::SplitCentrifugal,
            SvgShape::SplitOverlapping,
        ] {
            let png_variant = render_png(
                &analysis,
                &PngOptions {
                    width: 64,
                    height: 24,
                    shape: *shape,
                },
            )
            .expect("render png variant");
            assert!(png_variant.starts_with(b"\x89PNG\r\n\x1a\n"));
        }
    }
}
