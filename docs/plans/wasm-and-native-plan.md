# Plan: WASM + React Native Native Package

## Goal

Publish and maintain two npm packages from this workspace with aligned core behavior and versioning:

- `@moodbar/wasm` (existing): browser/node WebAssembly package.
- `@moodbar/native` (new): React Native package for iOS + Android with Expo compatibility.

Constraints for `@moodbar/native` v1:

- Async-only JS API.
- Inputs: file URI and in-memory bytes.
- Outputs: PNG bytes and SVG string, with platform-appropriate defaults.
- Option/name parity with existing moodbar options where practical.
- Package version tracks workspace/core version.

## Current Repository State (Scanned)

- Workspace crates: `moodbar-core`, `moodbar-cli`, `moodbar-wasm`.
- `moodbar-core` already supports feature-gated decode/png:
  - `decode` (`symphonia`) and `png` (`image`) are optional features.
- WASM packaging flow already exists:
  - `make wasm`, `make publish-check-wasm`
  - `scripts/prepare-npm-package.mjs`, `scripts/verify-npm-package.mjs`
  - CI workflows for wasm package validation and npm publish.
- Existing `docs/plans/wasm-and-native-plan.md` was based on `napi-rs` for RN, which is not the preferred Expo-compatible RN approach.

## Architecture Decision

Use **Rust core + C ABI FFI + Expo Module host bindings**.

- Rust remains source of truth for DSP/analysis/rendering.
- Swift/Kotlin are thin host layers for React Native bridge calls and threading.
- Expo Module gives best DX for Expo + bare RN with low integration friction.
- Keep RN New Architecture optional in v1; do not hard-require TurboModule/JSI.

Why not `napi-rs` for RN:

- Node-API bindings are for Node runtime targets.
- React Native compatibility and Expo developer ergonomics are better served by native module bindings (Expo Modules / RN bridge/TurboModule), not Node-API packaging.

## Target Package Layout

```text
crates/
  moodbar-core/
  moodbar-wasm/
  moodbar-native-ffi/        # new Rust crate exposing C ABI for mobile
packages/
  moodbar-native/            # new JS+native package (Expo module)
    src/
    ios/
    android/
    package.json
```

`moodbar-native-ffi` is internal and not published to crates.io.

## Native API Contract (v1)

JS-facing API (async-only):

```ts
export type MoodbarInput =
  | { uri: string }
  | { bytes: Uint8Array; name?: string; mimeType?: string };

export async function analyze(
  input: MoodbarInput,
  options?: AnalyzeOptions
): Promise<MoodbarAnalysis>;

export async function render(
  analysis: MoodbarAnalysis,
  format: "png" | "svg",
  options?: RenderOptions
): Promise<Uint8Array | string>;

export async function generate(
  input: MoodbarInput,
  format: "png" | "svg",
  options?: AnalyzeOptions & RenderOptions
): Promise<Uint8Array | string>;
```

Type-safe overloads to remove return ambiguity:

```ts
export async function render(
  analysis: MoodbarAnalysis,
  format: "png",
  options?: RenderOptions
): Promise<Uint8Array>;
export async function render(
  analysis: MoodbarAnalysis,
  format: "svg",
  options?: RenderOptions
): Promise<string>;

export async function generate(
  input: MoodbarInput,
  format: "png",
  options?: AnalyzeOptions & RenderOptions
): Promise<Uint8Array>;
export async function generate(
  input: MoodbarInput,
  format: "svg",
  options?: AnalyzeOptions & RenderOptions
): Promise<string>;
```

Behavioral defaults:

- Default image format in examples/docs: PNG.
- SVG returned as UTF-8 string.
- PNG returned as byte array (`Uint8Array`).

Parity intent:

- Reuse `GenerateOptions`, `SvgOptions`, `PngOptions` names and semantics where possible.
- If mobile-only options are needed later, add them as additive optional fields.

## Analysis Representation Decision

Use an **opaque native analysis handle** with explicit lifecycle management, not JSON/binary round-trip for v1.

- `analyze(...)` returns a JS object containing an internal handle ID managed by native code.
- `render(...)` accepts this handle and avoids re-serializing large frame arrays across bridge boundaries.
- JS never sees raw pointers; handle IDs map to native-owned analysis objects in a registry.
- Provide explicit `disposeAnalysis(handle)` for deterministic cleanup plus finalizer-backed fallback cleanup.

Rationale:

- Better performance and memory behavior than JSON/string re-serialization between `analyze` and `render`.
- Lower bridge payload size for common usage patterns.
- Keeps wire format private so we can introduce binary snapshots later without API breakage.

## Target Matrix and Tooling

Required Rust targets for mobile builds:

- iOS:
  - `aarch64-apple-ios`
  - `aarch64-apple-ios-sim`
  - `x86_64-apple-ios`
- Android:
  - `aarch64-linux-android`
  - `armv7-linux-androideabi`
  - `x86_64-linux-android`
  - `i686-linux-android`

Build tooling choices:

- `cbindgen` for generated C header(s) from `moodbar-native-ffi`.
- iOS static libs built per target via cargo + packaged into an `.xcframework` using `xcodebuild -create-xcframework`.
- `cargo-ndk` for Android multi-ABI builds and JNI libs packaging.
- `cargo-lipo` is not required when distributing `.xcframework`; prefer per-target archives + xcframework assembly.

## Implementation Phases

### Phase 1: Rust FFI substrate

1. Add `crates/moodbar-native-ffi`.
2. Expose minimal stable C ABI:
   - `analyze_from_path(...)`
   - `analyze_from_bytes(...)`
   - `render_png(...)`
   - `render_svg(...)`
   - `free_buffer(...)`/`free_analysis(...)`
3. Keep ownership/lifetimes explicit and panic-safe across FFI boundary.
4. Map Rust errors to structured error codes + message buffers.
5. Add deterministic ownership rules to API docs and binding code:
   - All FFI-returned buffers use `Box::into_raw`-style ownership transfer.
   - Every allocation has exactly one matching `free_*`.
   - Swift/Kotlin wrappers must free in `defer`/`finally` on success and error paths.
   - JS wrapper exposes `disposeAnalysis()` and also attaches finalizer fallback for leaked handles.

Notes:

- For in-memory input, decode from bytes via `symphonia` media stream path.
- Do not duplicate DSP logic; call into `moodbar-core` only.

### Phase 2: Expo module package

1. Scaffold `packages/moodbar-native` as Expo module.
2. iOS binding (Swift): call Rust static lib / xcframework wrappers on background queue.
3. Android binding (Kotlin): call Rust JNI/NDK bridge on background dispatcher.
4. Expose async methods only from JS surface.
5. Implement native handle registry used by `analyze` -> `render` -> `disposeAnalysis`.

### Phase 3: Packaging + release wiring

1. Add scripts mirroring wasm flow:
   - prepare package metadata from template
   - validate package contract files
2. Add `make native` and `make publish-check-native` targets.
3. Add CI jobs:
   - Linux job: Android NDK builds + package contract checks.
   - macOS job: iOS xcframework build + package contract checks.
   - Validate npm package contents with `npm pack --dry-run`.
4. Pin Android NDK version in CI and set `ANDROID_NDK_HOME` explicitly.
5. Add release publish workflow for `@moodbar/native` using npm trusted publishing.

### Phase 4: Stabilization

1. Golden tests for PNG/SVG output parity against CLI/core fixtures with explicit tolerance rules:
   - PNG: decode rendered bytes and compare RGBA pixel buffers exactly (ignore PNG file-level byte differences).
   - SVG: normalize and compare semantic structure (dimensions, stop count, ordered color stops) rather than raw string bytes.
2. Integration tests for both input modes (`uri`, `bytes`).
3. Error-contract tests (invalid input, decode failure, unsupported format).

## Early Quality Loop (Day-1, repeat often)

Use this loop during implementation to keep regressions small:

1. `make test-core`
   - validates DSP/analysis behavior while FFI is being added.
2. `cargo test -p moodbar --tests`
   - protects CLI behavior while core evolves.
3. `cargo check -p moodbar-native-ffi`
   - compile gate for FFI changes on every edit cycle.
4. `make check`
   - required workspace gate before PR/merge.

When `packages/moodbar-native` exists, extend loop:

1. `make native` (build package artifacts)
2. `make publish-check-native` (metadata + file contract + `npm pack --dry-run`)
3. RN smoke app run (Expo and bare): one API call each for `generate(..., "png")` and `generate(..., "svg")`.

## CI Additions (Planned)

Add new workflows modeled after existing wasm workflows:

- `native-ci.yml`
  - Trigger on native crate/package/script changes.
  - Linux runner job for Android NDK builds + package contract checks.
  - macOS runner job for iOS xcframework build + package contract checks.
- `publish-native-npm.yml`
  - On release publish.
  - Verify tag version equals workspace version.
  - Run `make publish-check-native`.
  - Publish `@moodbar/native` with provenance.

## Risks and Mitigations

1. FFI memory safety bugs.
   - Mitigation: strict `free_*` ownership contract, `defer`/`finally` freeing in bindings, finalizer fallback, sanitizer runs in CI where possible.
2. API drift from wasm/core options.
   - Mitigation: shared option schema docs + parity tests.
3. Mobile binary/toolchain complexity.
   - Mitigation: prebuilt artifacts in npm package for consumer DX, pinned Android NDK/tool versions in CI.
4. Expo compatibility regressions.
   - Mitigation: maintain minimal Expo example and run smoke tests in CI.

## Definition of Done (v1)

1. `@moodbar/native` installs in Expo and bare RN apps without requiring Rust toolchain.
2. Async API supports `uri` and `bytes` inputs.
3. PNG and SVG generation works on iOS + Android.
4. Option parity documented and tested against core defaults.
5. Native package publish check and release workflow pass.
