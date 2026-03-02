# Plan: WASM + React Native Native Module

## Goal

Extend this single Rust workspace to publish on **npm** and **crates.io** for two additional targets:

| Target             | Mechanism                                    | npm package                                      |
| ------------------ | -------------------------------------------- | ------------------------------------------------ |
| Browser / Node.js  | WebAssembly via `wasm-bindgen` + `wasm-pack` | `moodbar-wasm` (or `@username/moodbar-wasm`)     |
| React Native 0.76+ | Native module via `napi-rs` (Node-API)       | `moodbar-native` (or `@username/moodbar-native`) |

Both targets receive **already-decoded mono PCM** from JS and call the existing
`analyze_pcm_mono()` + rendering functions in `moodbar-core`. No file I/O is required
at the WASM or native binding layer (the native binding can optionally expose `analyze_path`).

**Package naming:** use unscoped `moodbar-wasm` / `moodbar-native` as placeholders;
switch to a personal npm scope (`@gildesmarais/...`) before publish if the unscoped names
are taken. No `@moodbar` org required.

**crates.io publishing targets:** `moodbar-core` (lib) and `moodbar-cli` (binary).
`moodbar-wasm` and `moodbar-napi` are not published to crates.io — they are deployed
to npm via their respective build tools.

---

## Current State

### Workspace layout

```
crates/
  moodbar-core/   lib: DSP, analysis, rendering — currently depends on symphonia + image
  moodbar-cli/    bin: generate / batch / inspect subcommands — uses rayon, walkdir
```

### Key public API in `moodbar-core/src/lib.rs`

**WASM-safe (no I/O, no threads):**

```rust
pub fn analyze_pcm_mono(sample_rate: u32, samples: &[f32], options: &GenerateOptions)
    -> MoodbarAnalysis

pub fn analysis_to_raw_rgb_bytes(analysis: &MoodbarAnalysis) -> Vec<u8>

pub fn render_svg(analysis: &MoodbarAnalysis, options: &SvgOptions) -> String

pub fn render_png(analysis: &MoodbarAnalysis, options: &PngOptions)
    -> Result<Vec<u8>, MoodbarError>   // requires `png` feature
```

**Not WASM-safe (filesystem + symphonia):**

```rust
pub fn analyze_path(path: &Path, options: &GenerateOptions)
    -> Result<MoodbarAnalysis, MoodbarError>

pub fn generate_moodbar_from_path(path: &Path, options: &GenerateOptions)
    -> Result<Vec<u8>, MoodbarError>
```

**Core structs (current, in lib.rs lines 18–146):**

```rust
pub struct GenerateOptions {
    pub fft_size: usize,            // default 2048
    pub low_cut_hz: f32,            // default 500.0
    pub mid_cut_hz: f32,            // default 2000.0
    pub normalize_mode: NormalizeMode,
    pub deterministic_floor: f64,   // default 1e-12
    pub detection_mode: DetectionMode,
    pub frames_per_color: usize,    // default 1
    pub band_edges_hz: Vec<f32>,    // default [500.0, 2000.0]
}

pub struct MoodbarAnalysis {
    pub channel_count: usize,
    pub frames: Vec<Vec<f64>>,      // [frame_idx][band_idx] in [0, 1]
    pub diagnostics: AnalysisDiagnostics,
}

pub struct SvgOptions { pub width: u32, pub height: u32, pub shape: SvgShape,
                        pub background: &'static str, pub max_gradient_stops: usize }
pub struct PngOptions { pub width: u32, pub height: u32, pub shape: SvgShape }

pub enum SvgShape { Strip, Waveform }
pub enum NormalizeMode { PerChannelPeak, GlobalPeak }
pub enum DetectionMode { SpectralEnergy, SpectralFlux }

pub enum MoodbarError {
    NoAudioTrack, EmptyAudio,
    Io(#[from] std::io::Error),
    Decode(#[from] SymphoniaError),   // symphonia-specific
    Image(#[from] image::ImageError), // image-specific
    InvalidOptions(String),
}
```

### Current `moodbar-core/Cargo.toml` dependencies

- `symphonia` — **direct dep**, used only in `analyze_path()` / `generate_moodbar_from_path()`
- `image` — **direct dep**, used only in `render_png()`
- `num-complex`, `rustfft`, `thiserror` — pure Rust, WASM-safe

### `moodbar-cli/Cargo.toml` dependencies (CLI-only, never needed for WASM/native)

- `rayon` — parallel batch; not in core
- `walkdir`, `clap`, `anyhow`, `serde`, `serde_json`

---

## Implementation Steps

### Step 1: Feature-gate `symphonia` and `image` in `moodbar-core`

**Why:** `moodbar-wasm` must not link against symphonia (codec decoding is not
WASM-compatible and adds significant binary size). The feature gate makes `moodbar-core`
itself WASM-compilable without any code changes to the DSP/rendering core.

**File: `crates/moodbar-core/Cargo.toml`**

Change symphonia and image to optional deps and add feature flags:

```toml
[features]
default = ["decode", "png"]
decode = ["dep:symphonia"]
png    = ["dep:image"]

[dependencies]
num-complex.workspace = true
rustfft.workspace = true
thiserror.workspace = true
symphonia  = { workspace = true, optional = true }
image      = { workspace = true, optional = true }
```

**File: `crates/moodbar-core/src/lib.rs`**

Gate imports at the top:

```rust
#[cfg(feature = "decode")]
use std::fs::File;
#[cfg(feature = "decode")]
use symphonia::core::{audio::SampleBuffer, codecs::DecoderOptions,
    errors::Error as SymphoniaError, formats::FormatOptions,
    io::MediaSourceStream, meta::MetadataOptions, probe::Hint};

#[cfg(feature = "png")]
use image::{ImageBuffer, ImageEncoder, Rgba};
```

Gate error variants:

```rust
pub enum MoodbarError {
    #[cfg(feature = "decode")]
    #[error("no playable audio track found")]
    NoAudioTrack,

    #[cfg(feature = "decode")]
    #[error("decoded stream has no samples")]
    EmptyAudio,

    #[cfg(feature = "decode")]
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[cfg(feature = "decode")]
    #[error("decode error: {0}")]
    Decode(#[from] SymphoniaError),

    #[cfg(feature = "png")]
    #[error("image error: {0}")]
    Image(#[from] image::ImageError),

    #[error("invalid options: {0}")]
    InvalidOptions(String),
}
```

Gate functions:

```rust
#[cfg(feature = "decode")]
pub fn analyze_path(path: &Path, options: &GenerateOptions)
    -> Result<MoodbarAnalysis, MoodbarError> { ... }

#[cfg(feature = "decode")]
pub fn generate_moodbar_from_path(path: &Path, options: &GenerateOptions)
    -> Result<Vec<u8>, MoodbarError> { ... }

#[cfg(feature = "png")]
pub fn render_png(analysis: &MoodbarAnalysis, options: &PngOptions)
    -> Result<Vec<u8>, MoodbarError> { ... }

#[cfg(feature = "png")]
pub struct PngOptions { ... }
```

**File: `crates/moodbar-cli/Cargo.toml`**

Explicitly enable both features (keep existing behavior):

```toml
[dependencies]
moodbar-core = { path = "../moodbar-core", features = ["decode", "png"] }
```

**Verify:** `cargo build -p moodbar-core --no-default-features` compiles cleanly.
`make check` (with default features) still passes all tests.

---

### Step 2: Create `crates/moodbar-wasm`

New crate. Thin `wasm-bindgen` wrapper. Depends on `moodbar-core` without `decode`.

**File: `crates/moodbar-wasm/Cargo.toml`**

```toml
[package]
name = "moodbar-wasm"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "WebAssembly bindings for moodbar audio visualization"

[lib]
crate-type = ["cdylib"]

[dependencies]
moodbar-core = { path = "../moodbar-core", default-features = false, features = ["png"] }
wasm-bindgen = "0.2"
serde = { workspace = true }
serde-wasm-bindgen = "0.6"

[dev-dependencies]
wasm-bindgen-test = "0.3"
```

Note: `png` feature is included so `render_png()` is available. The `image` crate with
`default-features = false, features = ["png"]` compiles cleanly to WASM (pure Rust PNG encoder).

**File: `crates/moodbar-wasm/src/lib.rs`**

Expose a flat JS-friendly API. Use `serde-wasm-bindgen` for complex types,
bare `#[wasm_bindgen]` on the entry point functions.

```rust
use moodbar_core::{
    analyze_pcm_mono, analysis_to_raw_rgb_bytes, render_svg, render_png,
    GenerateOptions, MoodbarAnalysis, SvgOptions, SvgShape, PngOptions,
    NormalizeMode, DetectionMode,
};
use wasm_bindgen::prelude::*;

/// Opaque handle to analysis result. Avoids copying frame data until needed.
#[wasm_bindgen]
pub struct WasmAnalysis(MoodbarAnalysis);

#[wasm_bindgen]
impl WasmAnalysis {
    pub fn frame_count(&self) -> usize { self.0.frames.len() }
    pub fn channel_count(&self) -> usize { self.0.channel_count }
}

/// Analyze mono PCM samples.
/// pcm: Float32Array from Web Audio API (AudioBuffer.getChannelData(0))
/// sample_rate: AudioBuffer.sampleRate (e.g. 44100)
#[wasm_bindgen]
pub fn analyze(pcm: &[f32], sample_rate: u32) -> WasmAnalysis {
    WasmAnalysis(analyze_pcm_mono(sample_rate, pcm, &GenerateOptions::default()))
}

/// Analyze with custom options. Pass a JS object; serde-wasm-bindgen handles conversion.
/// opts shape: { fft_size?, low_cut_hz?, mid_cut_hz?, frames_per_color?,
///               detection_mode?: "SpectralEnergy"|"SpectralFlux",
///               normalize_mode?: "PerChannelPeak"|"GlobalPeak" }
#[wasm_bindgen]
pub fn analyze_with_options(
    pcm: &[f32],
    sample_rate: u32,
    opts: JsValue,
) -> Result<WasmAnalysis, JsValue> {
    // Deserialize partial options from JS object, merge with defaults
    let options = js_opts_to_generate_options(opts)?;
    Ok(WasmAnalysis(analyze_pcm_mono(sample_rate, pcm, &options)))
}

/// Render SVG. Returns SVG string.
/// opts: { width?, height?, shape?: "Strip"|"Waveform", background?, max_gradient_stops? }
#[wasm_bindgen]
pub fn svg(analysis: &WasmAnalysis, opts: JsValue) -> Result<String, JsValue> {
    let options = js_opts_to_svg_options(opts)?;
    Ok(render_svg(&analysis.0, &options))
}

/// Render PNG. Returns Uint8Array of PNG bytes.
#[wasm_bindgen]
pub fn png(analysis: &WasmAnalysis, width: u32, height: u32) -> Result<Vec<u8>, JsValue> {
    let opts = PngOptions { width, height, shape: SvgShape::Strip };
    render_png(&analysis.0, &opts).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Legacy raw RGB bytes: [R0, G0, B0, R1, G1, B1, ...]
#[wasm_bindgen]
pub fn raw_rgb(analysis: &WasmAnalysis) -> Vec<u8> {
    analysis_to_raw_rgb_bytes(&analysis.0)
}
```

Helper functions `js_opts_to_generate_options()` and `js_opts_to_svg_options()` use
`serde_wasm_bindgen::from_value()` with local serde structs that have `#[serde(default)]`
on every field so partial JS objects work naturally.

**TypeScript definitions** are auto-generated by `wasm-pack`. The output will include
`moodbar_wasm.d.ts` with typed signatures.

**JS usage example (browser):**

```js
import init, { analyze, svg } from "moodbar-wasm";

await init();
const ctx = new AudioContext();
const buf = await ctx.decodeAudioData(arrayBuffer);
const pcm = buf.getChannelData(0); // mono Float32Array
const analysis = analyze(pcm, buf.sampleRate);
const svgStr = svg(analysis, { width: 800, height: 48, shape: "Waveform" });
document.getElementById("mood").innerHTML = svgStr;
```

---

### Step 3: Create `crates/moodbar-napi`

New crate. `napi-rs` v2 wrapper. Targets Node.js and React Native 0.76+ via Node-API.
Depends on `moodbar-core` with both `decode` and `png` features (native binary can decode files).

**File: `crates/moodbar-napi/Cargo.toml`**

```toml
[package]
name = "moodbar-napi"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Native Node.js/React Native bindings for moodbar audio visualization"
build = "build.rs"

[lib]
crate-type = ["cdylib"]

[dependencies]
moodbar-core = { path = "../moodbar-core", features = ["decode", "png"] }
napi        = { version = "2", default-features = false, features = ["napi4"] }
napi-derive = "2"

[build-dependencies]
napi-build = "2"
```

Use napi v2 (not v3) — v2 is the stable, widely-deployed version with best ecosystem support.
`napi4` feature covers async, threadsafe functions, and object handles.

**File: `crates/moodbar-napi/build.rs`**

```rust
fn main() {
    napi_build::setup();
}
```

**File: `crates/moodbar-napi/src/lib.rs`**

```rust
#![deny(clippy::all)]
use moodbar_core::{
    analyze_pcm_mono, analyze_path, analysis_to_raw_rgb_bytes, render_svg, render_png,
    GenerateOptions, MoodbarAnalysis, SvgOptions, SvgShape, PngOptions,
};
use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::path::Path;

/// Opaque handle to analysis. Avoids serializing the full frame data unless requested.
#[napi]
pub struct NapiAnalysis(MoodbarAnalysis);

#[napi]
impl NapiAnalysis {
    #[napi(getter)]
    pub fn frame_count(&self) -> u32 { self.0.frames.len() as u32 }

    #[napi(getter)]
    pub fn channel_count(&self) -> u32 { self.0.channel_count as u32 }

    /// Export raw frames as flat Float64Array [band0_f0, band1_f0, ..., band0_f1, ...]
    #[napi]
    pub fn frames_flat(&self) -> Vec<f64> {
        self.0.frames.iter().flat_map(|f| f.iter().copied()).collect()
    }
}

/// Analyze pre-decoded mono PCM.
/// pcm: Float32Array — no copy overhead, passed by reference via Node-API.
#[napi]
pub fn analyze_pcm(pcm: Float32Array, sample_rate: u32) -> NapiAnalysis {
    NapiAnalysis(analyze_pcm_mono(sample_rate, pcm.as_ref(), &GenerateOptions::default()))
}

/// Analyze an audio file (native only — uses symphonia decoder).
#[napi]
pub fn analyze_file(path: String) -> Result<NapiAnalysis> {
    analyze_path(Path::new(&path), &GenerateOptions::default())
        .map(NapiAnalysis)
        .map_err(|e| Error::from_reason(e.to_string()))
}

/// Render SVG string.
#[napi]
pub fn render_svg_string(analysis: &NapiAnalysis, width: u32, height: u32) -> String {
    let opts = SvgOptions { width, height, ..SvgOptions::default() };
    render_svg(&analysis.0, &opts)
}

/// Render PNG, returns Buffer.
#[napi]
pub fn render_png_buffer(analysis: &NapiAnalysis, width: u32, height: u32) -> Result<Buffer> {
    let opts = PngOptions { width, height, shape: SvgShape::Strip };
    render_png(&analysis.0, &opts)
        .map(|bytes| Buffer::from(bytes))
        .map_err(|e| Error::from_reason(e.to_string()))
}

/// Legacy raw RGB bytes as Buffer.
#[napi]
pub fn raw_rgb_buffer(analysis: &NapiAnalysis) -> Buffer {
    Buffer::from(analysis_to_raw_rgb_bytes(&analysis.0))
}
```

TypeScript definitions are auto-generated by `napi build`. The output `.node` binary is
platform-specific; publish one per platform target or use `@napi-rs/cli` to build matrix.

**React Native 0.76+ usage:**

```js
// React Native 0.76+ supports Node-API natively (New Architecture)
const moodbar = require("moodbar-native");

// Decode audio on native side (e.g. via expo-av or react-native-audio),
// then pass PCM Float32Array:
const analysis = moodbar.analyzePcm(float32Array, 44100);
const svg = moodbar.renderSvgString(analysis, 800, 48);
```

**Android cross-compilation (cargo-ndk):**

```sh
cargo ndk --target aarch64-linux-android --platform 21 -o android/app/src/main/jniLibs \
  -- build -p moodbar-napi --release
```

---

### Step 4: npm package scaffolding

Create a `packages/` directory (not a Rust workspace member — JS only):

```
packages/
  moodbar-wasm/
    # generated by wasm-pack; commit pkg/ or generate in CI
    # package.json, index.js, index_bg.wasm, moodbar_wasm.d.ts auto-generated
  moodbar-native/
    package.json
    index.js           # loads .node binary, re-exports typed API
    index.d.ts         # generated by napi-rs cli
```

**`packages/moodbar-native/package.json` skeleton:**

```json
{
  "name": "moodbar-native",
  "version": "0.1.0",
  "license": "MIT OR Apache-2.0",
  "main": "index.js",
  "types": "index.d.ts",
  "napi": {
    "name": "moodbar-napi",
    "triples": {
      "defaults": true,
      "additional": ["aarch64-linux-android", "aarch64-apple-ios"]
    }
  }
}
```

---

### Step 5: Workspace and tooling updates

**`Cargo.toml` (workspace root):**

Add new members:

```toml
[workspace]
members = [
  "crates/moodbar-core",
  "crates/moodbar-cli",
  "crates/moodbar-wasm",
  "crates/moodbar-napi",
]
```

Add new workspace deps:

```toml
[workspace.dependencies]
wasm-bindgen      = "0.2"
serde-wasm-bindgen = "0.6"
wasm-bindgen-test = "0.3"
napi              = { version = "2", default-features = false }
napi-derive       = "2"
napi-build        = "2"
```

**`Makefile`:**

```makefile
wasm:
	wasm-pack build crates/moodbar-wasm --release --target bundler \
	  --out-dir ../../packages/moodbar-wasm

napi:
	cd crates/moodbar-napi && napi build --platform --release \
	  --output-dir ../../packages/moodbar-native

napi-android:
	cargo ndk --target aarch64-linux-android --platform 21 \
	  -o packages/moodbar-native/android \
	  -- build -p moodbar-napi --release

check:
	cargo fmt --check
	cargo clippy --workspace --exclude moodbar-wasm -- -D warnings
	cargo test --workspace --exclude moodbar-wasm
	# WASM checked separately:
	cargo check -p moodbar-wasm --target wasm32-unknown-unknown
```

Note: `moodbar-wasm` is excluded from the default `clippy` and `test` runs because
it requires the WASM target. A separate CI job handles it.

**`.github/workflows/wasm-ci.yml`** (new file):

```yaml
name: WASM
on: [push, pull_request]
jobs:
  wasm:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown
      - run: cargo install wasm-pack
      - run: make wasm
```

---

## crates.io publishing checklist

Before publishing `moodbar-core` and `moodbar-cli` to crates.io:

1. Set real `repository` URL in `[workspace.package]` (currently `"https://example.invalid/moodbar"`)
2. Add `description` and `keywords` fields to each crate's `Cargo.toml`
3. Add `authors = ["Gil Desmarais"]`
4. Verify `cargo publish --dry-run -p moodbar-core`
5. Verify `cargo publish --dry-run -p moodbar` (the CLI)

---

## Dependency compatibility summary

| Dep                    | WASM              | Native (napi) | Notes                  |
| ---------------------- | ----------------- | ------------- | ---------------------- |
| `rustfft`              | ✓                 | ✓             | Pure Rust              |
| `num-complex`          | ✓                 | ✓             | Pure Rust              |
| `thiserror`            | ✓                 | ✓             | Macro crate            |
| `image` (png only)     | ✓                 | ✓             | Pure Rust PNG encoder  |
| `symphonia`            | ✗ (feature-gated) | ✓             | Not WASM-compilable    |
| `rayon`                | ✗ (CLI only)      | ✓             | Never in core          |
| `wasm-bindgen`         | ✓                 | N/A           | WASM binding layer     |
| `napi` / `napi-derive` | N/A               | ✓             | Node-API binding layer |

---

## Verification steps

1. `cargo build -p moodbar-core --no-default-features`
   → confirms core compiles without symphonia or image

2. `make check`
   → fmt + clippy + tests pass on CLI/core path (unchanged behavior)

3. `make wasm`
   → wasm-pack succeeds; `packages/moodbar-wasm/` contains `.wasm`, `.js`, `.d.ts`

4. Smoke test WASM in Node.js:

   ```js
   const { analyze, svg } = require("./packages/moodbar-wasm/moodbar_wasm.js");
   // pass a Float32Array of silence
   const a = analyze(new Float32Array(44100), 44100);
   console.log(svg(a, {})); // should output valid SVG
   ```

5. `make napi` (host platform)
   → `packages/moodbar-native/moodbar-napi.node` exists

6. Smoke test native:

   ```js
   const m = require("./packages/moodbar-native");
   const a = m.analyzePcm(new Float32Array(44100), 44100);
   console.log(m.renderSvgString(a, 800, 48).slice(0, 50)); // <svg ...
   ```

7. `cargo test --workspace --exclude moodbar-wasm`
   → all existing tests pass unchanged
