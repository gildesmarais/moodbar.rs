# Repository Guidelines

Rust rewrite of Moodbar. Legacy raw output (`R G B ...` bytes) must stay byte-compatible; `make parity` guards it.

## Crate rules

- `moodbar-analysis` owns all DSP and rendering (SVG/PNG). Keep it free of serde and symphonia. Add new output formats as renderers here over `MoodbarAnalysis`; never copy the pipeline.
- Option validation has one owner: `FrameAnalyzer::new` returns `Result<_, MoodbarError>`. Every entry point must go through it.
- `moodbar-decode` streams symphonia packets into `FrameAnalyzer`, downmixing to mono per packet. It never buffers the full PCM. WASM does not link it; the browser decodes.
- `moodbar-bindings-schema` owns serde option patches (`apply_*`) for WASM and FFI.
- `moodbar-core` is a pure re-export facade. Add no logic.
- `moodbar-native-ffi` is the C ABI for iOS and Android. It is not published to crates.io.
- Hot paths must not allocate per frame. `FrameAnalyzer` pre-allocates its buffers and builds the bin-to-band map once in `new`.

## FFI and mobile (`moodbar-native-ffi`, `packages/moodbar-native`)

- Never pass Rust pointers across the ABI. Analyses live in `registry.rs` as `Mutex<HashMap<u64, Arc<MoodbarAnalysis>>>`; only the `u64` handle crosses. Renders clone the `Arc` and release the lock before they run.
- Returned buffers are created by `mem::forget` into `MoodbarNativeBuffer` and must be freed exactly once with `moodbar_native_buffer_free`, which zeroes the struct. In Swift, put `defer { moodbar_native_buffer_free(&out) }` before the first `try` in the same scope. On failure the FFI never writes the out-param, which that pattern relies on.
- Wrap every `#[no_mangle]` export in `ffi_guard` (`errors.rs`), which uses `catch_unwind`. A panic becomes `MoodbarNativeStatus::Internal`; JNI exports reuse it through `guarded`. Only `moodbar_native_buffer_free` is unguarded: it cannot panic and must not clear the last error.
- Exports return `MoodbarNativeStatus` (0 = ok) and store the message in a thread-local. The host must call `moodbar_native_last_error` on the same thread right after a non-zero status, before any other FFI call.
- JNI (`android_jni.rs`) returns JSON envelopes rather than marshalling structs: `{"ok": true, ...}` or `{"ok": false, "status": N, "error": "..."}`. Kotlin throws `CodedException` when `ok` is false.
- iOS: lipo `aarch64-apple-ios-sim` and `x86_64-apple-ios` into one simulator slice before `xcodebuild -create-xcframework`, because XCFramework allows only one slice per platform variant.
- Build mobile artifacts with `--profile mobile-release`, never `--release`. It sets `panic = "unwind"` so `ffi_guard` can catch panics; the CLI `release` profile stays `panic = "abort"`.
- `build.rs` runs cbindgen and rewrites `include/moodbar_native_ffi.h`. After you add or change an export, build the crate and commit the header.

## Commands

- `make check`: fmt, clippy `-D warnings` and workspace tests. Required before commit.
- `make test-core` / `make tdd-core`: run `moodbar-analysis` and `moodbar-core` tests (`tdd-core` runs them in watch mode).
- `make test`, `make parity`.
- `make wasm` / `make publish-check-wasm`: npm package built with `--target bundler`. `make wasm-docs`: `--target web` build for `docs/`.
- `make native-ios` (needs macOS and Xcode), `make native-android` (needs NDK and cargo-ndk), `make publish-check-native[-ios|-android]`.
- Rust changes need tests; CLI contract changes need integration tests in `crates/moodbar-cli/tests`. FFI changes need at least `cargo check -p moodbar-native-ffi`.
- Errors: `thiserror` in libraries, `anyhow` in CLI and tests.
- `docs/plans/engineering-brief.md` is historical. Do not edit it.

## Default Code Instruction Plan

These apply to every change unless the user overrides one:

- **Zero backward-compat shims.** Add no adapters, translation layers or deprecation wrappers. Migrate all callers in the same commit.
- **Zero husks.** Remove the dead code, unused imports, orphaned modules and stale feature gates that the change touches.
- **Type-bound models.** Encode invariants in types (newtypes, enums, builders), not in runtime checks or strings.
- **SIMD/NEON in hot numeric paths** (FFT, normalization, color math). Gate it with `#[cfg(target_arch)]` and keep a scalar fallback.
- **Boy-scout LOC.** Each touched file ends with the same number of lines or fewer, unless the additions are new behavior with tests.
- **No assumptions** about files, env vars, features or platforms. Assert preconditions, and prefer compile-time checks to runtime checks.

## Commits

- Use Conventional Commits that state intent, not a file list. Keep each commit to one purpose, and separate perf changes from behavior changes.
- For cross-cutting changes, say why in the commit body and name the rollback boundary.
- Parity tests must tolerate missing legacy fixtures.
