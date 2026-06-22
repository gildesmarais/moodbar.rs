use thiserror::Error;

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
    pub frames: Vec<Vec<f64>>,
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

/// SVG output shape presets including split-band layouts.
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
    #[cfg(feature = "png")]
    #[error("image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("invalid options: {0}")]
    InvalidOptions(String),
}
