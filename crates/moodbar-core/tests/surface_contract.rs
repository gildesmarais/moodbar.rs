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

fn analysis_options_from_core(options: &core::GenerateOptions) -> analysis::GenerateOptions {
    analysis::GenerateOptions {
        fft_size: options.fft_size,
        low_cut_hz: options.low_cut_hz,
        mid_cut_hz: options.mid_cut_hz,
        normalize_mode: match options.normalize_mode {
            core::NormalizeMode::PerChannelPeak => analysis::NormalizeMode::PerChannelPeak,
            core::NormalizeMode::GlobalPeak => analysis::NormalizeMode::GlobalPeak,
        },
        deterministic_floor: options.deterministic_floor,
        detection_mode: match options.detection_mode {
            core::DetectionMode::SpectralEnergy => analysis::DetectionMode::SpectralEnergy,
            core::DetectionMode::SpectralFlux => analysis::DetectionMode::SpectralFlux,
        },
        frames_per_color: options.frames_per_color,
        band_edges_hz: options.band_edges_hz.clone(),
        max_target_frames: None,
        playback_rate: options.playback_rate,
        theme: match options.theme {
            core::Theme::Classic => analysis::Theme::Classic,
            core::Theme::Cool => analysis::Theme::Cool,
            core::Theme::Light => analysis::Theme::Light,
        },
        custom_colors: options.custom_colors.clone(),
    }
}

fn svg_shape_from_core(shape: core::SvgShape) -> analysis::SvgShape {
    match shape {
        core::SvgShape::Strip => analysis::SvgShape::Strip,
        core::SvgShape::Waveform => analysis::SvgShape::Waveform,
        core::SvgShape::SplitStacked => analysis::SvgShape::SplitStacked,
        core::SvgShape::SplitWaveform => analysis::SvgShape::SplitWaveform,
        core::SvgShape::SplitLanes => analysis::SvgShape::SplitLanes,
        core::SvgShape::SplitCentrifugal => analysis::SvgShape::SplitCentrifugal,
        core::SvgShape::SplitOverlapping => analysis::SvgShape::SplitOverlapping,
    }
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
    let analysis_analysis =
        analysis::analyze_pcm_mono(sample_rate, &pcm, &analysis_options_from_core(&options));

    assert_eq!(core_analysis.channel_count, analysis_analysis.channel_count);
    assert_eq!(core_analysis.frames.len(), analysis_analysis.frames.len());

    for (a, b) in core_analysis
        .frames
        .iter()
        .zip(analysis_analysis.frames.iter())
    {
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b.iter()) {
            assert!((x - y).abs() < 1e-12, "frame mismatch: {x} vs {y}");
        }
    }
}

#[test]
fn render_contract_matches_analysis_crate_for_all_shapes() {
    let sample_rate = 44_100;
    let pcm = synthetic_pcm(sample_rate);

    let core_analysis =
        core::analyze_pcm_mono(sample_rate, &pcm, &core::GenerateOptions::default());
    let analysis_analysis = analysis::analyze_pcm_mono(
        sample_rate,
        &pcm,
        &analysis::GenerateOptions {
            max_target_frames: None,
            ..analysis::GenerateOptions::default()
        },
    );

    let core_raw = core::analysis_to_raw_rgb_bytes(&core_analysis);
    let analysis_raw = analysis::analysis_to_raw_rgb_bytes(&analysis_analysis);
    assert_eq!(core_raw, analysis_raw);
    assert_eq!(core_analysis.colors(), analysis_analysis.colors());

    for shape in ALL_SHAPES {
        let core_svg = core::render_svg(
            &core_analysis,
            &core::SvgOptions {
                shape,
                ..core::SvgOptions::default()
            },
        );
        let analysis_svg = analysis::render_svg(
            &analysis_analysis,
            &analysis::SvgOptions {
                shape: svg_shape_from_core(shape),
                ..analysis::SvgOptions::default()
            },
        );
        assert_eq!(core_svg, analysis_svg, "svg mismatch for {shape:?}");

        let core_png = core::render_png(
            &core_analysis,
            &core::PngOptions {
                shape,
                ..core::PngOptions::default()
            },
        )
        .unwrap();
        let analysis_png = analysis::render_png(
            &analysis_analysis,
            &analysis::PngOptions {
                shape: svg_shape_from_core(shape),
                ..analysis::PngOptions::default()
            },
        )
        .unwrap();

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
