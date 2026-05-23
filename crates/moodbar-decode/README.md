# moodbar-decode

[![Crates.io](https://img.shields.io/crates/v/moodbar-decode.svg)](https://crates.io/crates/moodbar-decode)

The Symphonia-based audio decoding layer for the Moodbar visualization pipeline.

This crate is responsible for taking an audio file on disk, probing it using Symphonia, and decoding the track while streaming it directly into the `moodbar-analysis` DSP pipeline.

By keeping this separated from the DSP code (`moodbar-analysis`), you can omit this crate entirely if you are building for a platform where you already have decoded audio data (such as the browser with the Web Audio API, or native mobile apps).

## Usage Example

```rust
use moodbar_decode::analyze_path;
use moodbar_analysis::GenerateOptions;
use std::path::Path;

let path = Path::new("test.mp3");
let options = GenerateOptions::default();

// analyze_path decodes the file and runs the DSP pipeline
match analyze_path(path, &options) {
    Ok(analysis) => {
        println!("Generated {} frames of color data.", analysis.frames.len());
    }
    Err(e) => {
        eprintln!("Failed to decode audio: {}", e);
    }
}
```

## Supported Formats

Supports all major formats enabled by default in Symphonia, including `mp3`, `ogg` (Vorbis), `flac`, `wav`, `aac`, and `mp4`.
