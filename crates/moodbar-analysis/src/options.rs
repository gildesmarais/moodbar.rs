use crate::types::MoodbarError;

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
    pub max_target_frames: Option<usize>,
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
            max_target_frames: Some(2000),
            playback_rate: None,
            theme: Theme::Classic,
            custom_colors: None,
        }
    }
}

impl GenerateOptions {
    pub(crate) fn effective_band_edges(&self) -> Vec<f32> {
        if self.band_edges_hz.is_empty() {
            vec![self.low_cut_hz, self.mid_cut_hz]
        } else {
            self.band_edges_hz.clone()
        }
    }

    pub(crate) fn effective_nyquist_hz(&self, sample_rate: u32) -> f64 {
        let rate = self.playback_rate.unwrap_or(1.0) as f64;
        sample_rate as f64 / 2.0 * rate
    }

    /// Rejects options that would make analysis panic or produce garbage.
    /// Called once by `FrameAnalyzer::new`, the only analysis entry point.
    pub(crate) fn validate(&self, sample_rate: u32) -> Result<(), MoodbarError> {
        let fail = |msg: &str| Err(MoodbarError::InvalidOptions(msg.to_string()));
        if sample_rate == 0 {
            return fail("sample_rate must be greater than 0");
        }
        if !self.fft_size.is_power_of_two() || self.fft_size < 64 {
            return fail("fft_size must be a power of two and >= 64");
        }
        if !(self.deterministic_floor.is_finite() && self.deterministic_floor > 0.0) {
            return fail("deterministic_floor must be finite and > 0");
        }
        if self.frames_per_color == 0 {
            return fail("frames_per_color must be >= 1");
        }
        if self
            .playback_rate
            .is_some_and(|rate| !(rate.is_finite() && rate > 0.0))
        {
            return fail("playback_rate must be finite and > 0");
        }
        let edges = self.effective_band_edges();
        if !edges.iter().all(|edge| edge.is_finite()) {
            return fail("band edges must be finite");
        }
        if edges.windows(2).any(|pair| pair[0] >= pair[1]) {
            return fail("band edges must be strictly increasing");
        }
        Ok(())
    }
}

/// Parses a `#RRGGBB` or `RRGGBB` hex color.
///
/// # Errors
///
/// Returns `MoodbarError::InvalidOptions` unless the trimmed input is exactly six ASCII hex digits.
pub fn parse_hex_color(input: &str) -> Result<[u8; 3], MoodbarError> {
    let hex = input.trim().trim_start_matches('#');
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(MoodbarError::InvalidOptions(format!(
            "invalid color '{input}': expected 6 hex digits like #FF0000"
        )));
    }
    let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or_default();
    Ok([channel(0), channel(2), channel(4)])
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

// Rust guideline compliant 2026-06-22

#[cfg(test)]
mod tests {
    use super::*;

    fn rejects(options: GenerateOptions) -> String {
        match options.validate(44_100) {
            Err(MoodbarError::InvalidOptions(msg)) => msg,
            other => panic!("expected InvalidOptions, got {other:?}"),
        }
    }

    #[test]
    fn validate_rejects_non_finite_and_out_of_range_values() {
        let base = GenerateOptions::default;
        assert!(rejects(GenerateOptions {
            fft_size: 0,
            ..base()
        })
        .contains("fft_size"));
        assert!(rejects(GenerateOptions {
            band_edges_hz: vec![f32::NAN],
            ..base()
        })
        .contains("finite"));
        assert!(rejects(GenerateOptions {
            band_edges_hz: vec![],
            low_cut_hz: f32::INFINITY,
            ..base()
        })
        .contains("finite"));
        assert!(rejects(GenerateOptions {
            band_edges_hz: vec![900.0, 100.0],
            ..base()
        })
        .contains("increasing"));
        assert!(rejects(GenerateOptions {
            playback_rate: Some(f32::NAN),
            ..base()
        })
        .contains("playback_rate"));
        assert!(matches!(
            base().validate(0),
            Err(MoodbarError::InvalidOptions(_))
        ));
        assert!(base().validate(44_100).is_ok());
    }

    #[test]
    fn parse_hex_color_accepts_only_six_ascii_hex_digits() {
        assert_eq!(parse_hex_color(" #FF8000 ").unwrap(), [255, 128, 0]);
        assert_eq!(parse_hex_color("00ff10").unwrap(), [0, 255, 16]);
        for bad in ["", "#FFF", "GG0000", "+F+F+F", "€€", "#FF00001"] {
            assert!(parse_hex_color(bad).is_err(), "{bad:?} should be rejected");
        }
    }
}
