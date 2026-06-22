mod analyze;
mod bands;
mod options;
mod render;
mod types;

use analyze::frame_analyzer::FrameAnalyzer;
pub use options::{DetectionMode, GenerateOptions, NormalizeMode, Theme};
#[doc(inline)]
pub use render::{render_png, render_svg};
#[doc(inline)]
pub use types::{
    AnalysisDiagnostics, MoodbarAnalysis, MoodbarError, PngOptions, SvgOptions, SvgShape,
};

/// Analyze already-decoded mono PCM samples.
pub fn analyze_pcm_mono(
    sample_rate: u32,
    samples: &[f32],
    options: &GenerateOptions,
) -> MoodbarAnalysis {
    let mut analyzer = FrameAnalyzer::new(sample_rate, options, Some(samples.len()));
    analyzer.feed_mono_samples(samples);
    analyzer.finish()
}

pub fn analysis_to_raw_rgb_bytes(analysis: &MoodbarAnalysis) -> Vec<u8> {
    let mut out = Vec::<u8>::with_capacity(analysis.colors.len() * 3);
    for color in &analysis.colors {
        out.push(color[0]);
        out.push(color[1]);
        out.push(color[2]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq_hz: f32, sample_rate: u32, seconds: f32) -> Vec<f32> {
        let len = (sample_rate as f32 * seconds) as usize;
        (0..len)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                (2.0 * std::f32::consts::PI * freq_hz * t).sin()
            })
            .collect()
    }

    #[test]
    fn low_mid_high_have_distinct_dominant_channels() {
        let sample_rate = 44_100;
        let mut pcm = Vec::new();
        pcm.extend(sine(100.0, sample_rate, 0.5));
        pcm.extend(sine(1000.0, sample_rate, 0.5));
        pcm.extend(sine(5000.0, sample_rate, 0.5));

        let options = GenerateOptions::default();
        let analysis = analyze_pcm_mono(sample_rate, &pcm, &options);
        let bytes = analysis_to_raw_rgb_bytes(&analysis);
        let frame_count = bytes.len() / 3;
        assert!(frame_count > 10);

        let segment = frame_count / 3;
        let avg = |start: usize, end: usize| -> [f32; 3] {
            let mut sum = [0.0f32; 3];
            let count = (end - start) as f32;
            for i in start..end {
                sum[0] += bytes[i * 3] as f32;
                sum[1] += bytes[i * 3 + 1] as f32;
                sum[2] += bytes[i * 3 + 2] as f32;
            }
            [sum[0] / count, sum[1] / count, sum[2] / count]
        };

        let low = avg(0, segment);
        let mid = avg(segment, segment * 2);
        let high = avg(segment * 2, frame_count);

        assert!(low[0] > low[1] && low[0] > low[2]);
        assert!(mid[1] > mid[0] && mid[1] > mid[2]);
        assert!(high[2] > high[0] && high[2] > high[1]);
    }

    #[test]
    fn supports_more_than_three_bands() {
        let sample_rate = 44_100;
        let pcm = sine(400.0, sample_rate, 0.4);
        let options = GenerateOptions {
            band_edges_hz: vec![120.0, 500.0, 1200.0, 4000.0],
            ..GenerateOptions::default()
        };

        let analysis = analyze_pcm_mono(sample_rate, &pcm, &options);
        assert_eq!(analysis.channel_count, 5);
        assert!(!analysis.frames.is_empty());
    }

    #[test]
    fn frames_per_color_reduces_output_density() {
        let sample_rate = 44_100;
        let pcm = sine(400.0, sample_rate, 1.0);

        let baseline = analyze_pcm_mono(sample_rate, &pcm, &GenerateOptions::default());
        let dense = analyze_pcm_mono(
            sample_rate,
            &pcm,
            &GenerateOptions {
                frames_per_color: 1000,
                ..GenerateOptions::default()
            },
        );

        assert!(baseline.frames.len() > dense.frames.len());
        assert_eq!(dense.frames.len(), 1);
    }

    #[test]
    fn spectral_flux_reduces_steady_state_energy() {
        let sample_rate = 44_100;
        let pcm = sine(440.0, sample_rate, 1.0);

        let energy = analyze_pcm_mono(
            sample_rate,
            &pcm,
            &GenerateOptions {
                detection_mode: DetectionMode::SpectralEnergy,
                ..GenerateOptions::default()
            },
        );
        let flux = analyze_pcm_mono(
            sample_rate,
            &pcm,
            &GenerateOptions {
                detection_mode: DetectionMode::SpectralFlux,
                ..GenerateOptions::default()
            },
        );

        let energy_sum: f64 = energy.frames.iter().flatten().sum();
        let flux_sum: f64 = flux.frames.iter().flatten().sum();
        assert!(flux_sum < energy_sum);
    }

    #[test]
    fn test_playback_rate_frequency_shift() {
        let sample_rate = 44_100;
        let pcm = sine(1900.0, sample_rate, 1.0);
        let stable = GenerateOptions {
            max_target_frames: None,
            ..GenerateOptions::default()
        };

        let baseline = analyze_pcm_mono(sample_rate, &pcm, &stable);
        let faster = analyze_pcm_mono(
            sample_rate,
            &pcm,
            &GenerateOptions {
                playback_rate: Some(1.10),
                ..stable.clone()
            },
        );
        let slower = analyze_pcm_mono(
            sample_rate,
            &pcm,
            &GenerateOptions {
                playback_rate: Some(0.90),
                ..stable
            },
        );

        let total_band_energy = |analysis: &MoodbarAnalysis, band: usize| -> f64 {
            analysis.frames.iter().map(|frame| frame[band]).sum()
        };

        let high_baseline = total_band_energy(&baseline, 2);
        let high_faster = total_band_energy(&faster, 2);
        let low_baseline = total_band_energy(&baseline, 0);
        let low_slower = total_band_energy(&slower, 0);

        assert!(
            high_faster > high_baseline,
            "speed-up should shift energy toward high bands: {high_faster} vs {high_baseline}"
        );
        assert!(
            low_slower > low_baseline,
            "slow-down should shift energy toward low bands: {low_slower} vs {low_baseline}"
        );
    }

    #[test]
    fn svg_gradient_stop_count_is_capped() {
        let frames = (0..5000)
            .map(|i| {
                let t = i as f64 / 5000.0;
                vec![t, 1.0 - t, (0.5 + 0.5 * (t * 10.0).sin()).clamp(0.0, 1.0)]
            })
            .collect::<Vec<_>>();
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

    #[test]
    fn custom_themes_and_colors_work() {
        let sample_rate = 44_100;
        let pcm = sine(400.0, sample_rate, 0.5);

        // 1. Cool theme
        let options_cool = GenerateOptions {
            theme: Theme::Cool,
            ..GenerateOptions::default()
        };
        let analysis_cool = analyze_pcm_mono(sample_rate, &pcm, &options_cool);
        assert_eq!(
            analysis_cool.band_colors,
            vec![[220, 20, 180], [240, 120, 0], [0, 160, 240]]
        );

        // 2. Light theme
        let options_light = GenerateOptions {
            theme: Theme::Light,
            ..GenerateOptions::default()
        };
        let analysis_light = analyze_pcm_mono(sample_rate, &pcm, &options_light);
        assert_eq!(
            analysis_light.band_colors,
            vec![[240, 128, 128], [144, 238, 144], [173, 216, 230]]
        );

        // 3. Custom colors
        let options_custom = GenerateOptions {
            custom_colors: Some(vec![[255, 0, 255], [0, 255, 0], [255, 255, 0]]),
            ..GenerateOptions::default()
        };
        let analysis_custom = analyze_pcm_mono(sample_rate, &pcm, &options_custom);
        assert_eq!(
            analysis_custom.band_colors,
            vec![[255, 0, 255], [0, 255, 0], [255, 255, 0]]
        );

        // 4. Render standard SVG/PNG shapes with custom colors and check outputs
        let svg_strip = render_svg(
            &analysis_custom,
            &SvgOptions {
                shape: SvgShape::Strip,
                ..SvgOptions::default()
            },
        );
        // Ensure the custom color from analysis_custom.colors is present in the SVG strip
        let first_color = analysis_custom.colors[0];
        let expected_color = format!(
            "rgb({},{},{})",
            first_color[0], first_color[1], first_color[2]
        );
        assert!(svg_strip.contains(&expected_color));

        let svg_waveform = render_svg(
            &analysis_custom,
            &SvgOptions {
                shape: SvgShape::Waveform,
                ..SvgOptions::default()
            },
        );
        let boosted = crate::analyze::rgb_to_svg_rgb(first_color);
        let expected_stop = format!(
            "stop-color=\"rgb({},{},{})\"",
            boosted.0, boosted.1, boosted.2
        );
        assert!(svg_waveform.contains(&expected_stop));
    }
}
