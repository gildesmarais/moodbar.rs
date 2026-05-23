# moodbar-core

[![Crates.io](https://img.shields.io/crates/v/moodbar-core.svg)](https://crates.io/crates/moodbar-core)

A high-level convenience facade for the Moodbar visualization pipeline.

This crate currently implements the decoding and analysis logic natively to provide a unified, backwards-compatible interface for projects that were using `moodbar-core` prior to its split into discrete components.

If you are building a new project and only need DSP features (or only need decode features), it is recommended to depend on [`moodbar-analysis`](https://crates.io/crates/moodbar-analysis) or [`moodbar-decode`](https://crates.io/crates/moodbar-decode) directly to minimize dependencies.

## Usage Example

```rust
```rust
use moodbar_core::{analyze_path, GenerateOptions};
use std::path::Path;

let path = Path::new("test.mp3");

// 1. Analyze
let options = GenerateOptions::default();
let analysis = analyze_path(path, &options).unwrap();

println!("Generated {} frames", analysis.frames.len());
```
