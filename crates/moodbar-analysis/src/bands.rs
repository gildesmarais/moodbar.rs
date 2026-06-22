// Split-band display palette; matches legacy moodbar RGB band coloring.
pub(crate) const SPLIT_BAND_LOW_RGB: [u8; 3] = [220, 20, 180];
pub(crate) const SPLIT_BAND_MID_RGB: [u8; 3] = [240, 120, 0];
pub(crate) const SPLIT_BAND_HIGH_RGB: [u8; 3] = [0, 160, 240];
pub(crate) const SPLIT_OVERLAP_FILL_OPACITY: f64 = 0.40;
pub(crate) const SPLIT_OVERLAP_PNG_ALPHA: u8 = 102;

#[derive(Debug, Clone, Copy)]
pub(crate) struct SpectralBands {
    pub low: f64,
    pub mid: f64,
    pub high: f64,
}

impl SpectralBands {
    pub(crate) fn from_frame(frame: &[f64]) -> Self {
        match frame.len() {
            0 => Self {
                low: 0.0,
                mid: 0.0,
                high: 0.0,
            },
            1 => {
                let v = frame[0].clamp(0.0, 1.0);
                Self {
                    low: v,
                    mid: v,
                    high: v,
                }
            }
            2 => Self {
                low: frame[0].clamp(0.0, 1.0),
                mid: frame[1].clamp(0.0, 1.0),
                high: 0.0,
            },
            _ => Self {
                low: frame.first().copied().unwrap_or(0.0).clamp(0.0, 1.0),
                mid: frame.get(1).copied().unwrap_or(0.0).clamp(0.0, 1.0),
                high: frame.get(2).copied().unwrap_or(0.0).clamp(0.0, 1.0),
            },
        }
    }
}

pub(crate) fn scale_rgb(base: [u8; 3], energy: f64) -> [u8; 3] {
    [
        (base[0] as f64 * energy).round() as u8,
        (base[1] as f64 * energy).round() as u8,
        (base[2] as f64 * energy).round() as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_frame_single_channel_replicates() {
        let bands = SpectralBands::from_frame(&[0.75]);
        assert!((bands.low - 0.75).abs() < 1e-9);
        assert!((bands.mid - 0.75).abs() < 1e-9);
        assert!((bands.high - 0.75).abs() < 1e-9);
    }

    #[test]
    fn from_frame_two_channels_maps_low_mid() {
        let bands = SpectralBands::from_frame(&[0.5, 0.8]);
        assert!((bands.low - 0.5).abs() < 1e-9);
        assert!((bands.mid - 0.8).abs() < 1e-9);
        assert!((bands.high).abs() < 1e-9);
    }

    #[test]
    fn from_frame_three_channels_maps_bands() {
        let bands = SpectralBands::from_frame(&[1.0, 0.5, 0.25]);
        assert!((bands.low - 1.0).abs() < 1e-9);
        assert!((bands.mid - 0.5).abs() < 1e-9);
        assert!((bands.high - 0.25).abs() < 1e-9);
    }
}
