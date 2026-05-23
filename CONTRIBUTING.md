# Contributing to Moodbar.rs

This document outlines the workflows for developing, building, and publishing the crates and packages in this repository.

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

## Building Native Artifacts

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

## WASM Demo (Browser)

If you are developing the WASM module locally:
```bash
make wasm
python3 -m http.server
# open http://localhost:8000/docs/wasm-demo.html
```

## CI and Releases

All packages and releases are automatically published via GitHub Actions (with trusted publishing).

- CI workflow (Rust core): `.github/workflows/rust-ci.yml`
- CI workflow (WASM package): `.github/workflows/wasm-ci.yml`
- CI workflow (Native package): `.github/workflows/native-ci.yml`
- Release prep workflow: `.github/workflows/prepare-release.yml` (`workflow_dispatch`; opens PR that bumps `Cargo.toml` version)
- Release artifacts: `.github/workflows/release-build.yml` (Linux + macOS)
- Artifact naming: `moodbar-<tag>-<target>.tar.gz`
- npm release workflow: `.github/workflows/publish-wasm-npm.yml` (OIDC trusted publishing to [@moodbar/wasm](https://www.npmjs.com/package/@moodbar/wasm))
- npm release workflow (native): `.github/workflows/publish-native-npm.yml` (OIDC trusted publishing to [@moodbar/native](https://www.npmjs.com/package/@moodbar/native))
- Crates.io release workflow: Automatically publishes [moodbar](https://crates.io/crates/moodbar) and sub-crates.

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
