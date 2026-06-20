# moodbar-analysis

[![Crates.io](https://img.shields.io/crates/v/moodbar-analysis.svg)](https://crates.io/crates/moodbar-analysis)

Source of truth for moodbar DSP, FFT, normalization, and rendering. Takes decoded mono PCM, produces spectral frames and colors, then renders to raw RGB bytes, SVG, or PNG.

No audio decoding and no `serde` — suitable for WASM, FFI, and native Rust callers that supply PCM themselves.

## Module layout

| Module     | Role                                                              |
| ---------- | ----------------------------------------------------------------- |
| `options`  | `GenerateOptions`, `NormalizeMode`, `DetectionMode`               |
| `types`    | `MoodbarAnalysis`, `SvgOptions`, `PngOptions`, `SvgShape`, errors |
| `bands`    | `SpectralBands` (low/mid/high extraction for split renderers)     |
| `analyze/` | `FrameAnalyzer` streaming path, FFT, normalization, color mapping |
| `render/`  | `render_svg`, `render_png` (strip, waveform, split-band layouts)  |

## Features

- **`png`** (default): PNG rendering via the `image` crate.
- SVG rendering is always available (no feature gate).

## Public API

- `analyze_pcm_mono(sample_rate, samples, options)` — analyze pre-decoded mono PCM.
- `analysis_to_raw_rgb_bytes(analysis)` — legacy `R G B …` byte output.
- `render_svg(analysis, options)` — SVG markup.
- `render_png(analysis, options)` — PNG bytes (`png` feature).

Types re-exported from `options` and `types`: `GenerateOptions`, `MoodbarAnalysis`, `SvgOptions`, `PngOptions`, `SvgShape`, etc.

## SvgShape variants

| Variant            | Description                                     |
| ------------------ | ----------------------------------------------- |
| `Strip`            | Classic horizontal color strip (gradient fill). |
| `Waveform`         | Amplitude-shaped path with gradient stroke.     |
| `SplitStacked`     | Per-band stacked rectangles (low/mid/high).     |
| `SplitWaveform`    | Per-band waveform paths.                        |
| `SplitLanes`       | Horizontal lanes, one band per lane.            |
| `SplitCentrifugal` | Centrifugal split layout.                       |
| `SplitOverlapping` | Overlapping semi-transparent band layers.       |

Split SVG renderers emit CSS classes (`mood-low`, `mood-mid`, `mood-high`) for styling. Split layouts derive band energy from `SpectralBands` (first three channels when more bands are configured).

## Usage

```rust
use moodbar_analysis::{analyze_pcm_mono, render_svg, GenerateOptions, SvgOptions, SvgShape};

let sample_rate = 44_100;
let pcm = vec![0.0f32; sample_rate as usize];

let analysis = analyze_pcm_mono(sample_rate, &pcm, &GenerateOptions::default());

let svg = render_svg(
    &analysis,
    &SvgOptions {
        shape: SvgShape::SplitStacked,
        ..SvgOptions::default()
    },
);
```

For file-based decode, use [`moodbar-decode`](https://crates.io/crates/moodbar-decode) or decode PCM in your host (Web Audio API, platform codecs).
