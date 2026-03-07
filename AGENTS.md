# Repository Guidelines

## Purpose and Founding Decisions

This Rust workspace is a CLI-first rewrite of Moodbar with two explicit goals:

1. Keep practical compatibility with legacy raw moodbar output (`R G B ...` bytes).
2. Evolve beyond legacy constraints with extensible analysis and rendering.

Core design choices:

- Analysis-first architecture: decode/analyze once, then render to multiple formats.
- Pure Rust audio stack (`symphonia` + `rustfft`) for cross-platform portability.
- Shared DSP/rendering core across CLI, WASM, and native mobile targets.

### Workspace Crates

| Crate                     | Role                                                                                                                                                       |
| ------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `moodbar-analysis`        | DSP pipeline: FFT, normalization, frame aggregation, SVG/PNG rendering. No decode, no serde. Source of truth for all output behavior.                      |
| `moodbar-decode`          | Symphonia-based audio decode → mono PCM → delegates to `moodbar-analysis`. Not used in WASM (browser decodes). Always compiled with decode + PNG features. |
| `moodbar-bindings-schema` | Serde-deserialisable option patch types shared by WASM and FFI. Keeps serde out of `moodbar-analysis`.                                                     |
| `moodbar-core`            | Re-export facade over `moodbar-analysis` + `moodbar-decode`. Used by the CLI. Preserves backward compatibility; do not add new logic here.                 |
| `moodbar-cli`             | User-facing CLI: `generate`, `batch`, `inspect` subcommands.                                                                                               |
| `moodbar-wasm`            | WebAssembly bindings (wasm-bindgen). Receives pre-decoded PCM from the browser, calls `moodbar-analysis` directly.                                         |
| `moodbar-native-ffi`      | C ABI FFI layer for iOS and Android. Calls `moodbar-decode` + `moodbar-analysis`. Not published to crates.io.                                              |

### JavaScript/Native Packages

| Package           | Path                       | Role                                                            |
| ----------------- | -------------------------- | --------------------------------------------------------------- |
| `@moodbar/wasm`   | `crates/moodbar-wasm/pkg/` | Browser/Node WebAssembly package, built by `wasm-pack`.         |
| `@moodbar/native` | `packages/moodbar-native/` | Expo module for iOS + Android; ships prebuilt native artifacts. |

## Project Structure

```
crates/
  moodbar-analysis/src/lib.rs      DSP, FFT, normalization, SVG + PNG rendering, unit tests
  moodbar-decode/src/lib.rs        Symphonia decode pipeline, options validation
  moodbar-bindings-schema/src/lib.rs  Option patch types + apply_* helpers for JSON/JS options
  moodbar-core/src/lib.rs          Re-export facade (do not add logic here)
  moodbar-core/tests/
    legacy_parity.rs               Fixture-based byte-output parity harness
    surface_contract.rs            Asserts moodbar-core and moodbar-analysis produce identical output
  moodbar-cli/src/main.rs          generate, batch, inspect commands
  moodbar-cli/tests/svg_golden.rs  CLI integration/golden tests
  moodbar-wasm/src/lib.rs          wasm-bindgen bindings
  moodbar-native-ffi/
    src/lib.rs                     C ABI exports, buffer/summary types
    src/registry.rs                Opaque handle registry (Mutex<HashMap<u64, MoodbarAnalysis>>)
    src/errors.rs                  FfiError, MoodbarNativeStatus, ffi_guard, thread-local error
    src/options.rs                 JSON C-string → options parsing via moodbar-bindings-schema
    src/android_jni.rs             JNI entry points (compiled only on android target)
    build.rs                       cbindgen header generation
    include/moodbar_native_ffi.h   Generated C header (checked in)
packages/
  moodbar-native/
    index.js                       JS API: analyze, render, generate, disposeAnalysis
    index.d.ts                     TypeScript declarations with overloaded render/generate
    ios/MoodbarNativeModule.swift  Expo AsyncFunction bindings calling C ABI
    android/.../MoodbarNativeModule.kt  Expo AsyncFunction bindings calling JNI
    android/.../NativeBridge.kt    JNI external declarations + System.loadLibrary
scripts/
  build-native-ios.sh              Compile iOS targets, lipo simulator slices, create XCFramework
  build-native-android.sh          cargo-ndk cross-compile for four Android ABIs
  prepare-package.mjs              Inject version/repository into package.json
  verify-npm-package.mjs           Assert required files exist, run npm pack --dry-run
docs/plans/
  engineering-brief.md             Original founding decisions (historical, do not edit)
  wasm-and-native-plan.md          Architecture plan for the native package
```

## Algorithm and Performance Principles

- `moodbar-decode` collects all mono PCM samples into a `Vec<f32>`, then passes to `analyze_pcm_mono` in `moodbar-analysis`. The streaming path (chunked `feed_mono_samples`) lives in `FrameAnalyzer` inside `moodbar-analysis` and is used when the caller drives it directly (e.g. WASM, tests).
- Reuse FFT/frame scratch buffers in hot paths (avoid per-frame allocations); `FrameAnalyzer` pre-allocates all buffers.
- Precompute FFT-bin-to-band mapping once per `FrameAnalyzer::new`.
- Keep deterministic controls explicit (`normalize_mode`, `deterministic_floor`).
- For SVG, cap gradient stop count (`max_gradient_stops`) while preserving analysis precision.
- Do not duplicate DSP logic. All analysis and rendering goes through `moodbar-analysis`. New output formats should add a renderer there, not copy the pipeline.

## FFI and Mobile Conventions

These apply to any work touching `moodbar-native-ffi` or `packages/moodbar-native`.

### Handle registry — never pass raw Rust pointers across FFI

`MoodbarAnalysis` objects are stored in a process-global `Mutex<HashMap<u64, MoodbarAnalysis>>` with atomically-incremented `u64` handles. The handle is what crosses the FFI boundary. This avoids requiring the host language (Swift/Kotlin) to manage Rust object lifetimes.

The registry mutex is held for the duration of render operations. Renders are not concurrent in v1; this is a known contention point for future improvement.

### Buffer ownership

Heap buffers returned to the host are produced by:

```rust
std::mem::forget(bytes); // transfer ownership
*out_buffer = MoodbarNativeBuffer { ptr, len, cap };
```

They must be freed exactly once via `moodbar_native_buffer_free`, which reconstructs the `Vec` and drops it. The free function zeroes the struct after freeing to catch double-free in debug builds.

On the Swift side, always register `defer { moodbar_native_buffer_free(&out) }` **before** the first `try` in the same scope. On failure the FFI does not write to `out_buffer`, so the ptr stays null, but the pattern must be upheld for future correctness.

### Panic safety

All `#[no_mangle]` exports are wrapped in `ffi_guard`, which uses `std::panic::catch_unwind` to prevent Rust panics from unwinding across the C ABI boundary. A caught panic maps to `FfiError::Panic` → `MoodbarNativeStatus::Internal`.

### Error protocol

Exported functions return `MoodbarNativeStatus` (0 = ok). On failure, the error message is stored in a `thread_local`. The host must call `moodbar_native_last_error` immediately after a non-zero status on the same thread, before any subsequent FFI call overwrites the message.

### Android JNI envelope pattern

The JNI layer (`android_jni.rs`) does not use direct struct marshalling. Every JNI function returns a JSON string:

- Success: `{"ok": true, ...payload...}`
- Failure: `{"ok": false, "status": N, "error": "..."}`

Kotlin's `NativeBridge` parses this envelope; `MoodbarNativeModule` throws `CodedException` on `ok: false`. This avoids JNI struct complexity at the cost of a small JSON allocation per call — acceptable given audio decode dominates call time.

### iOS — C ABI direct

Swift calls the C functions in `moodbar_native_ffi.h` directly via the bridging header. The XCFramework bundles the static library for device (`aarch64-apple-ios`) and a fat simulator binary (lipo of `aarch64-apple-ios-sim` + `x86_64-apple-ios`). Both must be fat-merged before `xcodebuild -create-xcframework` — XCFramework allows only one slice per platform variant.

### Mobile release profile

Use `--profile mobile-release` for all mobile artifact builds, not `--release`:

```toml
[profile.mobile-release]
opt-level = "z"    # minimize binary size
lto = true
codegen-units = 1
panic = "abort"    # no unwinding tables
strip = true
```

### C header generation

`cbindgen` is run from `moodbar-native-ffi/build.rs` on every build and writes to `include/moodbar_native_ffi.h` in the source tree. The file is checked in. If you add a new `#[no_mangle]` export, build the crate locally and commit the updated header.

## Build, Test, and TDD Commands

```sh
# Core quality gate (required before commit)
make check              # fmt + clippy -D warnings + test

# Fast inner loops
make test-core          # moodbar-analysis + moodbar-core tests only
make test               # full workspace
make parity             # legacy fixture parity harness
make tdd-core           # watch mode on moodbar-analysis/core

# WASM package
make wasm               # build + prepare npm metadata
make publish-check-wasm # build + verify contract + npm pack --dry-run

# Native package (metadata only, no native build)
make native

# Native artifacts
make native-ios         # requires macOS + Xcode; builds XCFramework
make native-android     # requires Android NDK + cargo-ndk

# Native contract validation (depends on artifact builds)
make publish-check-native-ios
make publish-check-native-android
make publish-check-native   # both platforms, for local full validation
```

## Coding Style and Quality Bar

- Rust 2021, idiomatic ownership, explicit error propagation (`thiserror` for library errors, `anyhow` in tests/CLI).
- Prefer small, composable functions and data-oriented structs.
- No unchecked performance regressions in hot loops; profile-sensitive code should avoid hidden allocations.
- New behavior must ship with tests. CLI contract changes need integration tests. FFI changes need `cargo check -p moodbar-native-ffi` at minimum; prefer a unit test in the relevant module.
- `moodbar-analysis` must remain free of serde and symphonia. Use `moodbar-bindings-schema` for option deserialization in bindings layers.

## Commit and PR Expectations

- Use Conventional Commits with intent/rationale (not file-by-file narration).
- Keep commits single-purpose; performance changes and behavior changes should be isolated.
- Before commit: run `make check`.
- If scope is cross-cutting, state why and define rollback boundary in commit body.

## Notes for Future Work

- Legacy fixture generation may be environment-constrained; keep parity tests tolerant to missing fixtures.
- Additive format work (new image outputs, render variants) should add a renderer in `moodbar-analysis` and reuse `MoodbarAnalysis` — never duplicate DSP logic.
- The handle registry mutex serializes concurrent render calls. A future improvement could clone the `MoodbarAnalysis` out of the registry before releasing the lock, allowing concurrent renders on the same handle.
- Golden output tests for PNG/SVG parity across CLI, WASM, and native (Phase 4 of the native plan) are not yet implemented. When added, decide tolerance strategy upfront: pixel-exact hash or perceptual threshold.
- `moodbar-decode` buffers the full decoded PCM before passing to `analyze_pcm_mono`. For very long files this allocates proportionally to track duration. A streaming handoff between decode and analysis is a future optimization.
- `@moodbar/wasm` does not include audio decode (the browser's Web Audio API is used instead). If server-side Node.js decode is needed, it would require a separate `moodbar-wasm-node` target that links `moodbar-decode`.
