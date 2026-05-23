# moodbar-analysis

[![Crates.io](https://img.shields.io/crates/v/moodbar-analysis.svg)](https://crates.io/crates/moodbar-analysis)

The core DSP, FFT, and rendering pipeline for the Moodbar visualizer.

This crate is responsible for taking decoded mono PCM audio data, running the Fast Fourier Transform (using `rustfft`), and performing frequency band aggregation and normalization to produce the characteristic moodbar color sequences.

It operates entirely independent of any audio decoding logic (and is decoupled from `symphonia`), making it lightweight and suitable for use in WASM, FFI, and native Rust applications where you handle audio decoding yourself.

## Features

- **`png`**: Enables rendering the moodbar output directly to a PNG image (using the `image` crate).
- SVG rendering is included by default.

## Usage Example

```rust
use moodbar_analysis::{analyze_pcm_mono, GenerateOptions};

// In a real application, obtain PCM data from your audio decoder (e.g., moodbar-decode or Web Audio API)
let sample_rate = 44_100;
let pcm_data = vec![0.0f32; sample_rate as usize]; // 1 second of silence

let options = GenerateOptions::default();

// Run the analysis pipeline
let analysis = analyze_pcm_mono(sample_rate, &pcm_data, &options);

println!("Generated {} frames of color data.", analysis.frames.len());
```

## Rendering

You can use `analysis` to generate raw RGB bytes, SVG markup, or PNG bytes (with the `png` feature). All rendering paths are provided directly by this crate.
