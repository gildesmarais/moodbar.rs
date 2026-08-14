// Rust guideline compliant 2026-06-22

use image::load_from_memory;
use moodbar_analysis as analysis;
use moodbar_core as core;

fn sine(freq_hz: f32, sample_rate: u32, seconds: f32) -> Vec<f32> {
    let n = (sample_rate as f32 * seconds) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            (2.0 * std::f32::consts::PI * freq_hz * t).sin()
        })
        .collect()
}

fn synthetic_pcm(sample_rate: u32) -> Vec<f32> {
    let mut pcm = Vec::new();
    pcm.extend(sine(120.0, sample_rate, 0.4));
    pcm.extend(sine(900.0, sample_rate, 0.4));
    pcm.extend(sine(3300.0, sample_rate, 0.4));
    pcm
}

const ALL_SHAPES: [core::SvgShape; 7] = [
    core::SvgShape::Strip,
    core::SvgShape::Waveform,
    core::SvgShape::SplitStacked,
    core::SvgShape::SplitWaveform,
    core::SvgShape::SplitLanes,
    core::SvgShape::SplitCentrifugal,
    core::SvgShape::SplitOverlapping,
];

#[test]
fn analyze_pcm_contract_matches_analysis_crate() {
    let sample_rate = 44_100;
    let pcm = synthetic_pcm(sample_rate);

    let options = core::GenerateOptions {
        normalize_mode: core::NormalizeMode::GlobalPeak,
        detection_mode: core::DetectionMode::SpectralFlux,
        frames_per_color: 2,
        playback_rate: Some(1.09),
        ..core::GenerateOptions::default()
    };

    let core_analysis = core::analyze_pcm_mono(sample_rate, &pcm, &options);
    let analysis_analysis = analysis::analyze_pcm_mono(sample_rate, &pcm, &options);

    assert_eq!(core_analysis.channel_count, analysis_analysis.channel_count);
    assert_eq!(core_analysis.frames.len(), analysis_analysis.frames.len());
    assert_eq!(core_analysis.colors, analysis_analysis.colors);
    assert_eq!(core_analysis.frames, analysis_analysis.frames);
}

#[test]
fn render_contract_matches_analysis_crate_for_all_shapes() {
    let sample_rate = 44_100;
    let pcm = synthetic_pcm(sample_rate);

    let analysis_result =
        core::analyze_pcm_mono(sample_rate, &pcm, &core::GenerateOptions::default());

    let core_raw = core::analysis_to_raw_rgb_bytes(&analysis_result);
    let analysis_raw = analysis::analysis_to_raw_rgb_bytes(&analysis_result);
    assert_eq!(core_raw, analysis_raw);
    assert_eq!(analysis_result.colors(), analysis_result.colors.as_slice());

    for shape in ALL_SHAPES {
        let svg_opts = core::SvgOptions {
            shape,
            ..core::SvgOptions::default()
        };
        let core_svg = core::render_svg(&analysis_result, &svg_opts);
        let analysis_svg = analysis::render_svg(&analysis_result, &svg_opts);
        assert_eq!(core_svg, analysis_svg, "svg mismatch for {shape:?}");

        let png_opts = core::PngOptions {
            shape,
            ..core::PngOptions::default()
        };
        let core_png = core::render_png(&analysis_result, &png_opts).unwrap();
        let analysis_png = analysis::render_png(&analysis_result, &png_opts).unwrap();

        let core_img = load_from_memory(&core_png).unwrap().into_rgba8();
        let analysis_img = load_from_memory(&analysis_png).unwrap().into_rgba8();
        assert_eq!(
            core_img.dimensions(),
            analysis_img.dimensions(),
            "png dims for {shape:?}"
        );
        assert_eq!(
            core_img.as_raw(),
            analysis_img.as_raw(),
            "png pixels for {shape:?}"
        );
    }
}
