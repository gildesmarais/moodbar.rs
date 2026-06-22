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

    /// Validates the generate options, ensuring mathematical and physical constraints are met.
    ///
    /// # Errors
    ///
    /// Returns `MoodbarError::InvalidOptions` if validation fails.
    pub fn validate(&self) -> Result<(), crate::types::MoodbarError> {
        if !self.fft_size.is_power_of_two() || self.fft_size < 64 {
            return Err(crate::types::MoodbarError::InvalidOptions(
                "fft_size must be a power of two and >= 64".to_string(),
            ));
        }
        if !(self.deterministic_floor.is_finite() && self.deterministic_floor > 0.0) {
            return Err(crate::types::MoodbarError::InvalidOptions(
                "deterministic_floor must be finite and > 0".to_string(),
            ));
        }
        if self.frames_per_color == 0 {
            return Err(crate::types::MoodbarError::InvalidOptions(
                "frames_per_color must be >= 1".to_string(),
            ));
        }
        if let Some(rate) = self.playback_rate {
            if !(rate.is_finite() && rate > 0.0) {
                return Err(crate::types::MoodbarError::InvalidOptions(
                    "playback_rate must be finite and > 0".to_string(),
                ));
            }
        }

        let edges = if self.band_edges_hz.is_empty() {
            vec![self.low_cut_hz, self.mid_cut_hz]
        } else {
            self.band_edges_hz.clone()
        };

        if edges.is_empty() {
            return Err(crate::types::MoodbarError::InvalidOptions(
                "at least one band edge is required".to_string(),
            ));
        }
        for pair in edges.windows(2) {
            if pair[0] >= pair[1] {
                return Err(crate::types::MoodbarError::InvalidOptions(
                    "band edges must be strictly increasing".to_string(),
                ));
            }
        }
        Ok(())
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

// Rust guideline compliant 2026-06-22
