use moodbar_analysis::{
    DetectionMode, GenerateOptions, NormalizeMode, PngOptions, SvgOptions, SvgShape,
};
use serde::Deserialize;

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct GenerateOptionsPatch {
    pub fft_size: Option<usize>,
    pub low_cut_hz: Option<f32>,
    pub mid_cut_hz: Option<f32>,
    pub normalize_mode: Option<NormalizeModeInput>,
    pub deterministic_floor: Option<f64>,
    pub detection_mode: Option<DetectionModeInput>,
    pub frames_per_color: Option<usize>,
    pub band_edges_hz: Option<Vec<f32>>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct SvgOptionsPatch {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub shape: Option<SvgShapeInput>,
    pub background: Option<String>,
    pub max_gradient_stops: Option<usize>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct PngOptionsPatch {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub shape: Option<SvgShapeInput>,
}

#[derive(Deserialize)]
pub enum NormalizeModeInput {
    PerChannelPeak,
    GlobalPeak,
}

#[derive(Deserialize)]
pub enum DetectionModeInput {
    SpectralEnergy,
    SpectralFlux,
}

#[derive(Deserialize)]
pub enum SvgShapeInput {
    Strip,
    Waveform,
}

impl From<NormalizeModeInput> for NormalizeMode {
    fn from(value: NormalizeModeInput) -> Self {
        match value {
            NormalizeModeInput::PerChannelPeak => NormalizeMode::PerChannelPeak,
            NormalizeModeInput::GlobalPeak => NormalizeMode::GlobalPeak,
        }
    }
}

impl From<DetectionModeInput> for DetectionMode {
    fn from(value: DetectionModeInput) -> Self {
        match value {
            DetectionModeInput::SpectralEnergy => DetectionMode::SpectralEnergy,
            DetectionModeInput::SpectralFlux => DetectionMode::SpectralFlux,
        }
    }
}

impl From<SvgShapeInput> for SvgShape {
    fn from(value: SvgShapeInput) -> Self {
        match value {
            SvgShapeInput::Strip => SvgShape::Strip,
            SvgShapeInput::Waveform => SvgShape::Waveform,
        }
    }
}

pub fn apply_generate_patch(options: &mut GenerateOptions, patch: GenerateOptionsPatch) {
    if let Some(v) = patch.fft_size {
        options.fft_size = v;
    }
    if let Some(v) = patch.low_cut_hz {
        options.low_cut_hz = v;
    }
    if let Some(v) = patch.mid_cut_hz {
        options.mid_cut_hz = v;
    }
    if let Some(v) = patch.normalize_mode {
        options.normalize_mode = v.into();
    }
    if let Some(v) = patch.deterministic_floor {
        options.deterministic_floor = v;
    }
    if let Some(v) = patch.detection_mode {
        options.detection_mode = v.into();
    }
    if let Some(v) = patch.frames_per_color {
        options.frames_per_color = v;
    }
    if let Some(v) = patch.band_edges_hz {
        options.band_edges_hz = v;
    }
}

pub fn apply_svg_patch(options: &mut SvgOptions, patch: SvgOptionsPatch) -> Result<(), String> {
    if let Some(v) = patch.width {
        options.width = v;
    }
    if let Some(v) = patch.height {
        options.height = v;
    }
    if let Some(v) = patch.shape {
        options.shape = v.into();
    }
    if let Some(v) = patch.max_gradient_stops {
        options.max_gradient_stops = v;
    }
    if let Some(v) = patch.background {
        options.background = parse_svg_background(&v)?;
    }

    Ok(())
}

pub fn apply_png_patch(options: &mut PngOptions, patch: PngOptionsPatch) {
    if let Some(v) = patch.width {
        options.width = v;
    }
    if let Some(v) = patch.height {
        options.height = v;
    }
    if let Some(v) = patch.shape {
        options.shape = v.into();
    }
}

pub fn parse_svg_background(background: &str) -> Result<&'static str, String> {
    match background {
        "transparent" => Ok("transparent"),
        "black" => Ok("black"),
        "white" => Ok("white"),
        "none" => Ok("none"),
        _ => Err("unsupported background; use one of: transparent, black, white, none".to_string()),
    }
}
