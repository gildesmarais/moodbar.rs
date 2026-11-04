use moodbar_analysis::{
    parse_hex_color, DetectionMode, GenerateOptions, MoodbarError, NormalizeMode, PngOptions,
    SvgOptions, SvgShape, Theme,
};
use serde::{Deserialize, Serialize};

#[derive(Default, Deserialize, Serialize)]
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
    pub max_target_frames: Option<usize>,
    pub playback_rate: Option<f32>,
    pub theme: Option<ThemeInput>,
    pub custom_colors: Option<Vec<String>>,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(default)]
pub struct SvgOptionsPatch {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub shape: Option<SvgShapeInput>,
    pub background: Option<String>,
    pub max_gradient_stops: Option<usize>,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(default)]
pub struct PngOptionsPatch {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub shape: Option<SvgShapeInput>,
}

#[derive(Deserialize, Serialize)]
pub enum NormalizeModeInput {
    PerChannelPeak,
    GlobalPeak,
}

#[derive(Deserialize, Serialize)]
pub enum DetectionModeInput {
    SpectralEnergy,
    SpectralFlux,
}

#[derive(Deserialize, Serialize)]
pub enum SvgShapeInput {
    Strip,
    Waveform,
    SplitStacked,
    SplitWaveform,
    SplitLanes,
    SplitCentrifugal,
    SplitOverlapping,
}

#[derive(Deserialize, Serialize)]
pub enum ThemeInput {
    Classic,
    Cool,
    Light,
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
            SvgShapeInput::SplitStacked => SvgShape::SplitStacked,
            SvgShapeInput::SplitWaveform => SvgShape::SplitWaveform,
            SvgShapeInput::SplitLanes => SvgShape::SplitLanes,
            SvgShapeInput::SplitCentrifugal => SvgShape::SplitCentrifugal,
            SvgShapeInput::SplitOverlapping => SvgShape::SplitOverlapping,
        }
    }
}

impl From<ThemeInput> for Theme {
    fn from(value: ThemeInput) -> Self {
        match value {
            ThemeInput::Classic => Theme::Classic,
            ThemeInput::Cool => Theme::Cool,
            ThemeInput::Light => Theme::Light,
        }
    }
}

/// Applies `patch` onto `options`.
///
/// # Errors
///
/// Returns `MoodbarError::InvalidOptions` when a custom color is not valid hex.
pub fn apply_generate_patch(
    options: &mut GenerateOptions,
    patch: GenerateOptionsPatch,
) -> Result<(), MoodbarError> {
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
    if let Some(v) = patch.max_target_frames {
        options.max_target_frames = Some(v);
    }
    if let Some(v) = patch.playback_rate {
        options.playback_rate = Some(v);
    }
    if let Some(v) = patch.theme {
        options.theme = v.into();
    }
    if let Some(v) = patch.custom_colors {
        let colors = v
            .iter()
            .map(|s| parse_hex_color(s))
            .collect::<Result<_, _>>()?;
        options.custom_colors = Some(colors);
    }
    Ok(())
}

pub fn apply_svg_patch(
    options: &mut SvgOptions,
    patch: SvgOptionsPatch,
) -> Result<(), MoodbarError> {
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
        options.background = match v.as_str() {
            "transparent" => "transparent",
            "black" => "black",
            "white" => "white",
            "none" => "none",
            _ => {
                return Err(MoodbarError::InvalidOptions(
                    "unsupported background; use one of: transparent, black, white, none".into(),
                ))
            }
        };
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

#[cfg(test)]
mod tests {
    use super::*;
    use moodbar_analysis::GenerateOptions;

    #[test]
    fn apply_generate_patch_sets_playback_rate() {
        let patch: GenerateOptionsPatch =
            serde_json::from_str(r#"{"playback_rate": 1.09}"#).unwrap();
        let mut options = GenerateOptions::default();
        apply_generate_patch(&mut options, patch).unwrap();
        assert_eq!(options.playback_rate, Some(1.09));
    }

    #[test]
    fn apply_generate_patch_rejects_invalid_custom_color() {
        let patch: GenerateOptionsPatch =
            serde_json::from_str(r##"{"custom_colors": ["#FF0000", "not-hex"]}"##).unwrap();
        let mut options = GenerateOptions::default();
        assert!(matches!(
            apply_generate_patch(&mut options, patch),
            Err(MoodbarError::InvalidOptions(_))
        ));
        assert!(options.custom_colors.is_none());
    }

    #[test]
    fn apply_svg_patch_rejects_unknown_background() {
        let mut options = SvgOptions::default();
        let patch = |background: &str| SvgOptionsPatch {
            background: Some(background.into()),
            ..SvgOptionsPatch::default()
        };
        apply_svg_patch(&mut options, patch("white")).unwrap();
        assert_eq!(options.background, "white");
        assert!(matches!(
            apply_svg_patch(&mut options, patch("plaid")),
            Err(MoodbarError::InvalidOptions(_))
        ));
    }
}
