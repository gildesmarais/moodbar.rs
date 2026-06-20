# moodbar-native-ffi

C ABI FFI for iOS and Android native modules.

Bridges Swift/Kotlin to `moodbar-decode` (file/bytes → PCM → analysis) and `moodbar-analysis` (render). Generates `include/moodbar_native_ffi.h` via `cbindgen` on build.

## Flow

1. **Analyze** — decode audio, run analysis, store `MoodbarAnalysis` in a process-global registry, return a `u64` handle.
2. **Render** — pass handle + options; Rust renders SVG or PNG via `moodbar-analysis`.
3. **Dispose** — host removes the handle and frees analysis memory.

No raw Rust pointers cross the FFI boundary — only handles and owned buffers.

## Handle registry

```rust
static ANALYSIS_REGISTRY: Lazy<Mutex<HashMap<u64, Arc<MoodbarAnalysis>>>> = ...;
```

The registry mutex is held for the duration of render calls (renders are serialized in v1).

## Buffer ownership

Rendered PNG/SVG bytes are transferred with `std::mem::forget` into a `MoodbarNativeBuffer`. The host must free exactly once via `moodbar_native_buffer_free`.

On Swift: register `defer { moodbar_native_buffer_free(&out) }` before the first `try` in the same scope.

## Panic safety and errors

All `#[no_mangle]` exports use `ffi_guard` (`catch_unwind`). Failures return a non-zero `MoodbarNativeStatus`; call `moodbar_native_last_error()` on the same thread before the next FFI call.

## Android

JNI entry points return a JSON envelope (`{"ok": true, ...}` / `{"ok": false, "status", "error"}`) parsed by Kotlin `NativeBridge`.

## Render shapes

Svg/Png options accept the same `SvgShape` values as `moodbar-analysis`: `Strip`, `Waveform`, and the five split variants (`SplitStacked`, `SplitWaveform`, `SplitLanes`, `SplitCentrifugal`, `SplitOverlapping`).
