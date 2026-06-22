// Rust guideline compliant 2026-06-22

use wasm_bindgen_test::*;

use moodbar_bindings_schema::{
    GenerateOptionsPatch, PngOptionsPatch, SvgOptionsPatch, SvgShapeInput,
};
use moodbar_wasm::{analyze, analyze_with_options, png, raw_rgb, svg};

#[wasm_bindgen_test]
fn test_analyze_valid() {
    let pcm = vec![0.0f32; 4096];
    let res = analyze(&pcm, 44100);
    assert!(res.is_ok());
    let analysis = res.unwrap();
    assert_eq!(analysis.channel_count(), 3);
    assert!(analysis.frame_count() > 0);
}

#[wasm_bindgen_test]
fn test_analyze_zero_sample_rate() {
    let pcm = vec![0.0f32; 100];
    let res = analyze(&pcm, 0);
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(err.as_string().unwrap().contains("sample_rate"));
}

#[wasm_bindgen_test]
fn test_analyze_with_options_validation() {
    let pcm = vec![0.0f32; 4096];

    // Invalid fft_size (not a power of two)
    let opts = GenerateOptionsPatch {
        fft_size: Some(100),
        ..Default::default()
    };
    let js_opts = serde_wasm_bindgen::to_value(&opts).unwrap();
    let res = analyze_with_options(&pcm, 44100, js_opts.into());
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(err
        .as_string()
        .unwrap()
        .contains("fft_size must be a power of two"));

    // Valid custom options
    let opts_valid = GenerateOptionsPatch {
        fft_size: Some(256),
        frames_per_color: Some(10),
        ..Default::default()
    };
    let js_opts_valid = serde_wasm_bindgen::to_value(&opts_valid).unwrap();
    let res_valid = analyze_with_options(&pcm, 44100, js_opts_valid.into());
    assert!(res_valid.is_ok());
}

#[wasm_bindgen_test]
fn test_render_svg_and_png() {
    let pcm = vec![0.1f32; 8192];
    let analysis = analyze(&pcm, 44100).unwrap();

    // Render SVG
    let svg_opts = SvgOptionsPatch {
        width: Some(600),
        height: Some(64),
        shape: Some(SvgShapeInput::Waveform),
        ..Default::default()
    };
    let js_svg_opts = serde_wasm_bindgen::to_value(&svg_opts).unwrap();
    let svg_res = svg(&analysis, js_svg_opts.into());
    assert!(svg_res.is_ok());
    let svg_str = svg_res.unwrap();
    assert!(svg_str.contains("<svg"));
    assert!(svg_str.contains("width=\"600\""));
    assert!(svg_str.contains("height=\"64\""));

    // Render PNG
    let png_opts = PngOptionsPatch {
        width: Some(100),
        height: Some(20),
        shape: Some(SvgShapeInput::SplitStacked),
    };
    let js_png_opts = serde_wasm_bindgen::to_value(&png_opts).unwrap();
    let png_res = png(&analysis, js_png_opts.into());
    assert!(png_res.is_ok());
    let png_bytes = png_res.unwrap();
    assert!(!png_bytes.is_empty());

    // Raw RGB
    let rgb = raw_rgb(&analysis);
    assert_eq!(rgb.len() % 3, 0);
}
