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
- `crates/moodbar-wasm`: WebAssembly JS bindings for browser/Node usage
- `tests/fixtures/legacy`: optional parity fixtures
- `scripts/`: helper scripts

## WASM Demo (Browser)

```bash
make wasm
python3 -m http.server
# open http://localhost:8000/docs/wasm-demo.html
```

## CI and Releases

- CI workflow: `.github/workflows/rust-ci.yml`
- Release artifacts: `.github/workflows/release-build.yml` (Linux + macOS)
- Artifact naming: `moodbar-<tag>-<target>.tar.gz`
- npm release workflow: `.github/workflows/publish-wasm-npm.yml` (OIDC trusted publishing)

## Publish WASM Package

```bash
# build and normalize npm package metadata/files
make wasm

# reproducibility and package contract checks
make publish-check-wasm

# publish manually (maintainer workflow)
npm publish ./crates/moodbar-wasm/pkg --access public
```
