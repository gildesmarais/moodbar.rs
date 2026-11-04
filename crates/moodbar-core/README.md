# moodbar-core

[![Crates.io](https://img.shields.io/crates/v/moodbar-core.svg)](https://crates.io/crates/moodbar-core)

Re-export facade used by the CLI: `pub use moodbar_analysis::*` plus `moodbar_decode::*` when the `decode` feature is enabled. It holds no logic of its own.

New projects should depend on [`moodbar-analysis`](https://crates.io/crates/moodbar-analysis) and/or [`moodbar-decode`](https://crates.io/crates/moodbar-decode) directly to avoid pulling duplicate DSP dependencies.

## Features

- **`decode`** (default): Symphonia-based `analyze_path` / `analyze_bytes`.
- **`png`** (default): PNG rendering (forwards to `moodbar-analysis`).

## Usage

```rust
use moodbar_core::{analyze_path, render_svg, GenerateOptions, SvgOptions, SvgShape};
use std::path::Path;

let path = Path::new("test.mp3");
let options = GenerateOptions::default();

let analysis = analyze_path(path, &options).unwrap();
println!("Generated {} frames", analysis.frames.len());

let svg = render_svg(
    &analysis,
    &SvgOptions {
        shape: SvgShape::Waveform,
        ..SvgOptions::default()
    },
);
```

`SvgShape` includes split-band variants: `SplitStacked`, `SplitWaveform`, `SplitLanes`, `SplitCentrifugal`, `SplitOverlapping`.
