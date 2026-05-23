# moodbar-native-ffi

The C ABI FFI layer for mobile native modules (iOS and Android).

This crate exposes `#[no_mangle]` C functions that bridge the host languages (Swift/Kotlin) to the `moodbar-decode` and `moodbar-analysis` crates. It generates a C header (`include/moodbar_native_ffi.h`) automatically during the build using `cbindgen`.

## Handle Registry (Thread Safety)

To avoid forcing Swift or Kotlin to manage Rust object lifetimes across the FFI boundary, this crate uses a process-global handle registry:

```rust
static ANALYSIS_REGISTRY: Lazy<Mutex<HashMap<u64, Arc<MoodbarAnalysis>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
```

1. **Analysis:** The `analyze()` export decodes the audio, runs the FFT, and stores the resulting `MoodbarAnalysis` in the registry. It returns a `u64` handle.
2. **Rendering:** Subsequent calls (like `render_png()`) pass the `u64` handle back to Rust. Rust acquires the mutex, looks up the analysis object, and generates the output.
3. **Disposal:** The host is responsible for calling the disposal function with the handle when it's done, which removes the entry from the map and frees the memory.

## Buffer Ownership Rules

Heap buffers (like rendered PNG bytes) returned to the host follow strict ownership transfer rules:

```rust
std::mem::forget(bytes); // transfer ownership
*out_buffer = MoodbarNativeBuffer { ptr, len, cap };
```

The host (Swift/Kotlin) **must** free this buffer exactly once by passing the struct back to `moodbar_native_buffer_free()`. On the Swift side, always register `defer { moodbar_native_buffer_free(&out) }` immediately before the first `try` in the same scope to ensure memory is not leaked upon failure.

## Panic Safety and Errors

All FFI exports are wrapped in an `ffi_guard` (`std::panic::catch_unwind`) to prevent panics from crossing the C ABI boundary. A caught panic maps to a non-zero status code. On failure, the host must call `moodbar_native_last_error()` on the same thread to retrieve the string description of the error.
