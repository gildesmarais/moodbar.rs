use moodbar_analysis::{
    analysis_to_raw_rgb_bytes, analyze_pcm_mono, render_png, render_svg, GenerateOptions,
    MoodbarAnalysis, PngOptions, SvgOptions, SvgShape,
};
use moodbar_bindings_schema::{
    apply_generate_patch, apply_svg_patch, GenerateOptionsPatch, SvgOptionsPatch,
};
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

fn js_opts_to_generate_options(opts: JsValue) -> Result<GenerateOptions, JsValue> {
    let parsed: GenerateOptionsPatch = decode_opts(opts)?;
    let mut options = GenerateOptions::default();
    apply_generate_patch(&mut options, parsed);
    Ok(options)
}

fn js_opts_to_svg_options(opts: JsValue) -> Result<SvgOptions, JsValue> {
    let parsed: SvgOptionsPatch = decode_opts(opts)?;
    let mut options = SvgOptions::default();
    apply_svg_patch(&mut options, parsed).map_err(|e| JsValue::from_str(&e))?;
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
