# Moodbar (Rust)

CLI-first moodbar generator in Rust.

## Prerequisites

- Rust toolchain (stable)
- `make`
- Node.js (required for `make wasm` / npm package preparation)

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
- `examples/web-wasm`: minimal browser integration example
- `examples/expo-native`: minimal Expo/React Native integration example
- `tests/fixtures/legacy`: optional parity fixtures
- `scripts/`: helper scripts

## WASM Demo (Browser)

```bash
make wasm
python3 -m http.server
# open http://localhost:8000/docs/wasm-demo.html
```

## React Native Package

`@moodbar/native` ships Expo-compatible native bindings for iOS + Android.
Native artifacts are built with the Cargo `mobile-release` profile (`opt-level=z`, `lto`, `strip`) to reduce binary size.

```bash
# prepare npm metadata/assets
make native

# build iOS xcframework (macOS/Xcode)
make native-ios

# build Android JNI libs (requires Android NDK + cargo-ndk)
make native-android
```

## CI and Releases

- CI workflow (Rust core): `.github/workflows/rust-ci.yml`
- CI workflow (WASM package): `.github/workflows/wasm-ci.yml`
- CI workflow (Native package): `.github/workflows/native-ci.yml`
- Release prep workflow: `.github/workflows/prepare-release.yml` (`workflow_dispatch`; opens PR that bumps `Cargo.toml` version)
- Release artifacts: `.github/workflows/release-build.yml` (Linux + macOS)
- Artifact naming: `moodbar-<tag>-<target>.tar.gz`
- npm release workflow: `.github/workflows/publish-wasm-npm.yml` (OIDC trusted publishing)
- npm release workflow (native): `.github/workflows/publish-native-npm.yml` (OIDC trusted publishing)

## Publish WASM Package

```bash
# build and normalize npm package metadata/files
make wasm

# reproducibility and package contract checks
make publish-check-wasm

# publish manually (maintainer workflow)
npm publish ./crates/moodbar-wasm/pkg --access public --provenance
```

## Publish Native Package

```bash
# build platform artifacts + prepare metadata/files
make native-ios
make native-android

# validate package contract and dry-run publish
make publish-check-native

# publish manually (maintainer workflow)
npm publish ./packages/moodbar-native --access public --provenance
```
