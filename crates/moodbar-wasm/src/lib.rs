use moodbar_core::{
    analysis_to_raw_rgb_bytes, analyze_pcm_mono, render_png, render_svg, DetectionMode,
    GenerateOptions, MoodbarAnalysis, NormalizeMode, PngOptions, SvgOptions, SvgShape,
};
use serde::Deserialize;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmAnalysis(MoodbarAnalysis);

#[wasm_bindgen]
impl WasmAnalysis {
    pub fn frame_count(&self) -> usize {
        self.0.frames.len()
    }

    pub fn channel_count(&self) -> usize {
        self.0.channel_count
    }
}

#[wasm_bindgen]
pub fn analyze(pcm: &[f32], sample_rate: u32) -> WasmAnalysis {
    WasmAnalysis(analyze_pcm_mono(
        sample_rate,
        pcm,
        &GenerateOptions::default(),
    ))
}

#[wasm_bindgen]
pub fn analyze_with_options(
    pcm: &[f32],
    sample_rate: u32,
    opts: JsValue,
) -> Result<WasmAnalysis, JsValue> {
    let options = js_opts_to_generate_options(opts)?;
    Ok(WasmAnalysis(analyze_pcm_mono(sample_rate, pcm, &options)))
}

#[wasm_bindgen]
pub fn svg(analysis: &WasmAnalysis, opts: JsValue) -> Result<String, JsValue> {
    let options = js_opts_to_svg_options(opts)?;
    Ok(render_svg(&analysis.0, &options))
}

#[wasm_bindgen]
pub fn png(analysis: &WasmAnalysis, width: u32, height: u32) -> Result<Vec<u8>, JsValue> {
    let opts = PngOptions {
        width,
        height,
        shape: SvgShape::Strip,
    };

    render_png(&analysis.0, &opts).map_err(|e| JsValue::from_str(&e.to_string()))
}

#[wasm_bindgen]
pub fn raw_rgb(analysis: &WasmAnalysis) -> Vec<u8> {
    analysis_to_raw_rgb_bytes(&analysis.0)
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct GenerateOptionsInput {
    fft_size: Option<usize>,
    low_cut_hz: Option<f32>,
    mid_cut_hz: Option<f32>,
    normalize_mode: Option<NormalizeModeInput>,
    deterministic_floor: Option<f64>,
    detection_mode: Option<DetectionModeInput>,
    frames_per_color: Option<usize>,
    band_edges_hz: Option<Vec<f32>>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct SvgOptionsInput {
    width: Option<u32>,
    height: Option<u32>,
    shape: Option<SvgShapeInput>,
    background: Option<String>,
    max_gradient_stops: Option<usize>,
}

#[derive(Deserialize)]
enum NormalizeModeInput {
    PerChannelPeak,
    GlobalPeak,
}

#[derive(Deserialize)]
enum DetectionModeInput {
    SpectralEnergy,
    SpectralFlux,
}

#[derive(Deserialize)]
enum SvgShapeInput {
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

fn js_opts_to_generate_options(opts: JsValue) -> Result<GenerateOptions, JsValue> {
    let parsed: GenerateOptionsInput = decode_opts(opts)?;
    let mut options = GenerateOptions::default();

    if let Some(v) = parsed.fft_size {
        options.fft_size = v;
    }
    if let Some(v) = parsed.low_cut_hz {
        options.low_cut_hz = v;
    }
    if let Some(v) = parsed.mid_cut_hz {
        options.mid_cut_hz = v;
    }
    if let Some(v) = parsed.normalize_mode {
        options.normalize_mode = v.into();
    }
    if let Some(v) = parsed.deterministic_floor {
        options.deterministic_floor = v;
    }
    if let Some(v) = parsed.detection_mode {
        options.detection_mode = v.into();
    }
    if let Some(v) = parsed.frames_per_color {
        options.frames_per_color = v;
    }
    if let Some(v) = parsed.band_edges_hz {
        options.band_edges_hz = v;
    }

    Ok(options)
}

fn js_opts_to_svg_options(opts: JsValue) -> Result<SvgOptions, JsValue> {
    let parsed: SvgOptionsInput = decode_opts(opts)?;
    let mut options = SvgOptions::default();

    if let Some(v) = parsed.width {
        options.width = v;
    }
    if let Some(v) = parsed.height {
        options.height = v;
    }
    if let Some(v) = parsed.shape {
        options.shape = v.into();
    }
    if let Some(v) = parsed.max_gradient_stops {
        options.max_gradient_stops = v;
    }
    if let Some(v) = parsed.background {
        options.background = parse_svg_background(&v)?;
    }

    Ok(options)
}

fn decode_opts<T>(opts: JsValue) -> Result<T, JsValue>
where
    T: for<'de> Deserialize<'de> + Default,
{
    if opts.is_undefined() || opts.is_null() {
        return Ok(T::default());
    }

    serde_wasm_bindgen::from_value(opts)
        .map_err(|e| JsValue::from_str(&format!("invalid options object: {e}")))
}

fn parse_svg_background(background: &str) -> Result<&'static str, JsValue> {
    match background {
        "transparent" => Ok("transparent"),
        "black" => Ok("black"),
        "white" => Ok("white"),
        "none" => Ok("none"),
        _ => Err(JsValue::from_str(
            "unsupported background; use one of: transparent, black, white, none",
        )),
    }
}
