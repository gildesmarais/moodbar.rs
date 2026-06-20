# moodbar-bindings-schema

[![Crates.io](https://img.shields.io/crates/v/moodbar-bindings-schema.svg)](https://crates.io/crates/moodbar-bindings-schema)

Serde-deserializable option patches for WASM and native FFI bindings.

Keeps `serde` out of `moodbar-analysis`. Patch types merge into `moodbar_analysis::GenerateOptions`, `SvgOptions`, and `PngOptions` via `apply_*` helpers.

## Generate options (`GenerateOptionsPatch`)

| Field                 | Type              | Notes                                                                                        |
| --------------------- | ----------------- | -------------------------------------------------------------------------------------------- |
| `fft_size`            | integer           | Default in analysis: `2048` (power of two, ≥ 64).                                            |
| `normalize_mode`      | string            | `PerChannelPeak` (default) or `GlobalPeak`.                                                  |
| `deterministic_floor` | float             | Default: `1e-12`.                                                                            |
| `detection_mode`      | string            | `SpectralEnergy` (default) or `SpectralFlux`.                                                |
| `frames_per_color`    | integer           | Default: `1`.                                                                                |
| `band_edges_hz`       | float array       | Default: `[500.0, 2000.0]` (3 bands). When empty, falls back to `low_cut_hz` / `mid_cut_hz`. |
| `max_target_frames`   | integer, optional | Caps output frames by adjusting hop size. Default in analysis: `2000`.                       |
| `playback_rate`       | float, optional   | Scales band mapping for pitch-shifted playback. Must be finite and > 0.                      |

## Render options

**SVG** (`SvgOptionsPatch`): `width`, `height`, `shape`, `background` (`transparent` \| `black` \| `white` \| `none`), `max_gradient_stops`.

**PNG** (`PngOptionsPatch`): `width`, `height`, `shape`.

### SvgShape values

`Strip`, `Waveform`, `SplitStacked`, `SplitWaveform`, `SplitLanes`, `SplitCentrifugal`, `SplitOverlapping`.

Example JSON:

```json
{
  "fft_size": 4096,
  "normalize_mode": "GlobalPeak",
  "shape": "SplitWaveform"
}
```
