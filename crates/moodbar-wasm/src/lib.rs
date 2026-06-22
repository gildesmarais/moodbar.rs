// Rust guideline compliant 2026-06-22

use moodbar_analysis::{
    analysis_to_raw_rgb_bytes, analyze_pcm_mono, render_png, render_svg, GenerateOptions,
    MoodbarAnalysis, PngOptions, SvgOptions,
};
use moodbar_bindings_schema::{
    apply_generate_patch, apply_png_patch, apply_svg_patch, GenerateOptionsPatch, PngOptionsPatch,
    SvgOptionsPatch,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(typescript_custom_section)]
const TS_OPTIONS_DEFS: &str = r#"
export interface GenerateOptions {
    fft_size?: number;
    low_cut_hz?: number;
    mid_cut_hz?: number;
    normalize_mode?: 'PerChannelPeak' | 'GlobalPeak';
    deterministic_floor?: number;
    detection_mode?: 'SpectralEnergy' | 'SpectralFlux';
    frames_per_color?: number;
    band_edges_hz?: number[];
    max_target_frames?: number;
    playback_rate?: number;
    theme?: 'Classic' | 'Cool' | 'Light';
    custom_colors?: string[];
}

export interface SvgOptions {
    width?: number;
    height?: number;
    shape?: 'Strip' | 'Waveform' | 'SplitStacked' | 'SplitWaveform' | 'SplitLanes' | 'SplitCentrifugal' | 'SplitOverlapping';
    background?: 'transparent' | 'black' | 'white' | 'none';
    max_gradient_stops?: number;
}

export interface PngOptions {
    width?: number;
    height?: number;
    shape?: 'Strip' | 'Waveform' | 'SplitStacked' | 'SplitWaveform' | 'SplitLanes' | 'SplitCentrifugal' | 'SplitOverlapping';
}
"#;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "GenerateOptions")]
    pub type GenerateOptionsJs;

    #[wasm_bindgen(typescript_type = "SvgOptions")]
    pub type SvgOptionsJs;

    #[wasm_bindgen(typescript_type = "PngOptions")]
    pub type PngOptionsJs;
}

#[wasm_bindgen]
#[derive(Debug)]
pub struct WasmAnalysis(MoodbarAnalysis);

#[wasm_bindgen]
impl WasmAnalysis {
    pub fn frame_count(&self) -> usize {
        self.0.frames.len() / self.0.channel_count.max(1)
    }

    pub fn channel_count(&self) -> usize {
        self.0.channel_count
    }
}

#[wasm_bindgen(start)]
pub fn init_panic_hook() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub fn analyze(pcm: &[f32], sample_rate: u32) -> Result<WasmAnalysis, JsValue> {
    if sample_rate == 0 {
        return Err(JsValue::from_str("sample_rate must be greater than 0"));
    }
    Ok(WasmAnalysis(analyze_pcm_mono(
        sample_rate,
        pcm,
        &GenerateOptions::default(),
    )))
}

#[wasm_bindgen]
pub fn analyze_with_options(
    pcm: &[f32],
    sample_rate: u32,
    opts: GenerateOptionsJs,
) -> Result<WasmAnalysis, JsValue> {
    if sample_rate == 0 {
        return Err(JsValue::from_str("sample_rate must be greater than 0"));
    }
    let options = js_opts_to_generate_options(opts.into())?;
    Ok(WasmAnalysis(analyze_pcm_mono(sample_rate, pcm, &options)))
}

#[wasm_bindgen]
pub fn svg(analysis: &WasmAnalysis, opts: SvgOptionsJs) -> Result<String, JsValue> {
    let options = js_opts_to_svg_options(opts.into())?;
    Ok(render_svg(&analysis.0, &options))
}

#[wasm_bindgen]
pub fn png(analysis: &WasmAnalysis, opts: PngOptionsJs) -> Result<Vec<u8>, JsValue> {
    let options = js_opts_to_png_options(opts.into())?;
    render_png(&analysis.0, &options).map_err(|e| JsValue::from_str(&e.to_string()))
}

#[wasm_bindgen]
pub fn raw_rgb(analysis: &WasmAnalysis) -> Vec<u8> {
    analysis_to_raw_rgb_bytes(&analysis.0)
}

fn js_opts_to_generate_options(opts: JsValue) -> Result<GenerateOptions, JsValue> {
    let parsed: GenerateOptionsPatch = decode_opts(opts)?;
    let mut options = GenerateOptions::default();
    apply_generate_patch(&mut options, parsed);
    validate_generate_options(&options)?;
    Ok(options)
}

fn validate_generate_options(options: &GenerateOptions) -> Result<(), JsValue> {
    options
        .validate()
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

fn js_opts_to_svg_options(opts: JsValue) -> Result<SvgOptions, JsValue> {
    let parsed: SvgOptionsPatch = decode_opts(opts)?;
    let mut options = SvgOptions::default();
    apply_svg_patch(&mut options, parsed).map_err(|e| JsValue::from_str(&e))?;
    Ok(options)
}

fn js_opts_to_png_options(opts: JsValue) -> Result<PngOptions, JsValue> {
    let parsed: PngOptionsPatch = decode_opts(opts)?;
    let mut options = PngOptions::default();
    apply_png_patch(&mut options, parsed);
    Ok(options)
}

fn decode_opts<T>(opts: JsValue) -> Result<T, JsValue>
where
    T: for<'de> serde::Deserialize<'de> + Default,
{
    if opts.is_undefined() || opts.is_null() {
        return Ok(T::default());
    }

    serde_wasm_bindgen::from_value(opts)
        .map_err(|e| JsValue::from_str(&format!("invalid options object: {e}")))
}
