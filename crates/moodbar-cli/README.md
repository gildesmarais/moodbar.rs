# moodbar (CLI)

[![Crates.io](https://img.shields.io/crates/v/moodbar.svg)](https://crates.io/crates/moodbar)

A CLI-first moodbar generator in Rust. It takes audio files and generates visual timelines (moodbars) based on the audio characteristics, supporting both legacy raw output (for compatibility with Amarok-era tools) and modern SVG/PNG rendering.

## Installation

```bash
cargo install moodbar
```

## Quick Start

```bash
# generate legacy raw moodbar bytes (.mood)
moodbar generate -i input.ogg -o output.mood

# generate SVG output
moodbar generate -i input.ogg -o output.svg --format svg --svg-shape waveform

# generate PNG output
moodbar generate -i input.ogg -o output.png --format png

# inspect a moodbar file
moodbar inspect -i output.mood
```

## Advanced Options

Common tuning flags include `--normalize-mode`, `--deterministic-floor`, `--detection-mode`, `--frames-per-color`, and `--band-edges-hz`.
Use command help for full details:

```bash
moodbar generate --help
moodbar batch --help
```

## Batch Mode

```bash
moodbar batch -i ./music -o ./moods --progress
```

## Related Crates

This CLI is a frontend for the `moodbar-core` library. If you are building your own application, you can use the underlying library crates:

- [`moodbar-analysis`](https://crates.io/crates/moodbar-analysis) - DSP, FFT, normalization, and rendering (SVG/PNG)
- [`moodbar-decode`](https://crates.io/crates/moodbar-decode) - Symphonia-based audio decoding
