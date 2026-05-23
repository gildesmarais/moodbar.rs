# moodbar-bindings-schema

[![Crates.io](https://img.shields.io/crates/v/moodbar-bindings-schema.svg)](https://crates.io/crates/moodbar-bindings-schema)

The shared options schema and serialization logic for the Moodbar visualizer.

This crate exposes data structures (like `GenerateOptions`) that are used across all bindings (CLI, WebAssembly, and Native FFI) to configure the DSP pipeline without polluting the core `moodbar-analysis` crate with `serde` dependencies.

## Advanced Configuration Options

When interacting with the WASM or Native bindings, you can pass a JSON string to configure the analysis. The core configurable fields are:

- **`fft_size`** (integer, default: 2048) - The size of the FFT window.
- **`normalize_mode`** (string) - Determines how peak energy is clamped.
  - `"PerChannelPeak"` (default): Normalizes each channel independently.
  - `"GlobalPeak"`: Preserves relative loudness between channels.
- **`deterministic_floor`** (float, default: 1e-12) - The silence floor threshold.
- **`detection_mode`** (string) - The math used for band aggregation.
  - `"SpectralEnergy"` (default)
  - `"SpectralFlux"`
- **`frames_per_color`** (integer, default: 1) - How many FFT frames aggregate into a single output color block.
- **`band_edges_hz`** (array of floats) - Custom frequency band edges for the low/mid/high split. Defaults to `[0.0, 500.0, 2000.0, 22050.0]`.
- **`max_target_frames`** (integer, optional) - Limits the total number of output frames by dynamically adjusting the FFT hop size. Useful for preventing excessive memory usage on long tracks.

Example JSON payload:

```json
{
  "fft_size": 4096,
  "normalize_mode": "GlobalPeak"
}
```
