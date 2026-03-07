# Macro Refactor Plan

## Goal

Move the project from feature-gated multi-surface coupling toward explicit crate boundaries, shared API contracts, and cross-surface correctness checks.

Primary outcomes:

- Clear separation of DSP/rendering vs media decoding concerns.
- One shared bindings schema for CLI/WASM/native surfaces.
- Smaller, testable native FFI modules.
- Strong contract tests that prevent output/API drift.
- Simpler package/release script surface.

## Priority Order

1. Crate split: `moodbar-analysis` + `moodbar-decode`.
2. Shared bindings schema crate.
3. `moodbar-native-ffi` module decomposition.
4. Cross-surface contract test harness.
5. Packaging script unification.
6. Stable typed JS error model.
7. End-user examples workspace (Expo + Web).

## Phase 1: Core Crate Boundary Refactor

### Scope

- Introduce `crates/moodbar-analysis`:
  - FFT/frame analysis
  - normalization
  - `MoodbarAnalysis`
  - SVG/PNG rendering primitives
- Introduce `crates/moodbar-decode`:
  - `symphonia`-based decode from path/bytes
  - decode diagnostics
- Keep a compatibility `crates/moodbar-core` facade for one transition cycle:
  - re-export old APIs
  - mark legacy paths with migration comments

### Why first

This removes feature-gating pressure from a single crate and makes target-specific dependency boundaries explicit.

### Acceptance

- `moodbar-wasm` depends on `moodbar-analysis` only.
- `moodbar-native-ffi` and CLI depend on both analysis+decode where needed.
- `make check` passes with no behavior regressions.

## Phase 2: Shared Bindings Schema

### Scope

- Add `crates/moodbar-bindings-schema` with serde models for:
  - analyze options
  - render options
  - enums (`NormalizeMode`, `DetectionMode`, `SvgShape`)
  - validation helpers
- Replace duplicated parse/mapping logic in:
  - `crates/moodbar-wasm/src/lib.rs`
  - `crates/moodbar-native-ffi/src/lib.rs`

### Acceptance

- No duplicate option-mapping implementations in wasm/native.
- API option names stay parity-aligned across surfaces.
- Existing wasm/native checks pass.

## Phase 3: Native FFI Internal Decomposition

### Scope

Split `crates/moodbar-native-ffi/src/lib.rs` into:

- `abi.rs` (extern C functions + structs)
- `registry.rs` (analysis handle lifecycle)
- `options.rs` (JSON parsing/validation)
- `errors.rs` (status mapping + last error)
- `android_jni.rs` (Android bridge only)

### Acceptance

- Public ABI unchanged.
- File-level responsibilities are single-purpose.
- `cargo clippy -p moodbar-native-ffi --all-targets -- -D warnings` passes.

## Phase 4: Cross-Surface Contract Harness

### Scope

Add fixture-driven contract tests that run equivalent operations through CLI, wasm, and native paths and compare:

- analysis metadata (`frame_count`, `channel_count`)
- PNG decoded RGBA pixel buffers
- normalized SVG semantic structure

### Acceptance

- Contract test suite runs in CI.
- Any drift in options/output semantics fails fast.

## Phase 5: Packaging/Release Script Unification

### Scope

- Replace specialized prep scripts with one generic script:
  - e.g. `scripts/prepare-package.mjs`
  - config-driven per package (`@moodbar/wasm`, `@moodbar/native`)
- Keep a single pattern for:
  - version sync
  - required docs/licenses
  - package contract verification hooks

### Acceptance

- One prep entry point used by `make wasm` and `make native`.
- Current publish-check targets still pass.

## Phase 6: Stable Typed Error Model for JS Users

### Scope

- Define stable error codes across wasm/native:
  - `INVALID_INPUT`
  - `DECODE_FAILED`
  - `RENDER_FAILED`
  - `INTERNAL`
- Ensure JS-facing APIs throw structured errors with code + message.

### Acceptance

- Docs show code-based error handling examples.
- Integration tests verify code stability for representative failures.

## Phase 7: Examples Workspace

### Scope

- Add `examples/expo-native` and `examples/web-wasm`.
- Keep them minimal but executable as smoke checks.
- Reuse these examples in CI sanity checks where practical.

### Acceptance

- Example apps build/run with current packages.
- Examples stay aligned with published API.

## Execution and Risk Controls

- Keep commits single-purpose by phase or sub-phase.
- Preserve backward-compatible facade until migration complete.
- Run `make check` after each meaningful sub-step.
- Run package contract checks (`make publish-check-wasm`, `make publish-check-native`) at phase boundaries.

## Suggested Delivery Cadence

1. Phase 1 + 2 as the first vertical refactor milestone.
2. Phase 3 + 4 as hardening milestone.
3. Phase 5 + 6 + 7 as release/DX milestone.

## Definition of Complete

- Explicit crate boundaries are enforced by dependencies, not feature flags.
- wasm/native share one option schema source.
- native FFI internals are modular and testable.
- cross-surface contract tests protect correctness.
- package/release flow is unified and minimal.
- users get stable, typed errors and runnable examples.
