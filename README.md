# Moodbar (Rust)

CLI-first moodbar generator in Rust.

## Prerequisites
- Rust toolchain (stable)
- `make`

## Install
```bash
cargo install --path crates/moodbar-cli
```

## Quick Start
```bash
# run the full local quality gate
make check

# generate legacy raw moodbar bytes (.mood)
cargo run -p moodbar -- generate -i input.ogg -o output.mood

# generate SVG output
cargo run -p moodbar -- generate -i input.ogg -o output.svg --format svg --svg-shape waveform

# inspect a moodbar file
cargo run -p moodbar -- inspect -i output.mood
```

For installed usage, replace `cargo run -p moodbar --` with `moodbar`.

## Advanced Options
Common tuning flags include `--normalize-mode`, `--deterministic-floor`, `--detection-mode`, `--frames-per-color`, and `--band-edges-hz`.
Use command help for full details:
```bash
moodbar generate --help
moodbar batch --help
```

## Developer Workflow
```bash
# core crate fast loop
make test-core

# full workspace tests
make test

# parity harness (skips when fixtures are absent)
make parity

# fmt + clippy -D warnings + tests
make check

# optional watch loop
make tdd-core
```

## Batch Mode
```bash
cargo run -p moodbar -- batch -i ./music -o ./moods --progress
```

## Repository Layout
- `crates/moodbar-core`: decode, analysis, normalization, render primitives
- `crates/moodbar-cli`: `generate`, `batch`, `inspect` commands
- `tests/fixtures/legacy`: optional parity fixtures
- `scripts/`: helper scripts

## CI and Releases
- CI workflow: `.github/workflows/rust-ci.yml`
- Release artifacts: `.github/workflows/release-build.yml` (Linux + macOS)
- Artifact naming: `moodbar-<tag>-<target>.tar.gz`
