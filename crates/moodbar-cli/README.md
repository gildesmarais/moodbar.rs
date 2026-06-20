# moodbar (CLI)

[![Crates.io](https://img.shields.io/crates/v/moodbar.svg)](https://crates.io/crates/moodbar)

CLI-first moodbar generator. Reads audio files and writes legacy raw bytes (`.mood`), SVG, or PNG.

Built on `moodbar-core` (decode + analysis API, rendering delegated to `moodbar-analysis`).

## Installation

```bash
cargo install moodbar
```

## Quick start

```bash
# legacy raw moodbar bytes (.mood)
moodbar generate -i input.ogg -o output.mood

# SVG — classic strip or waveform
moodbar generate -i input.ogg -o output.svg --format svg --svg-shape waveform

# SVG — split-band layout (low/mid/high lanes)
moodbar generate -i input.ogg -o split.svg --format svg --svg-shape split-stacked

# PNG
moodbar generate -i input.ogg -o output.png --format png --svg-shape split-waveform

# inspect a moodbar file
moodbar inspect -i output.mood
```

## Svg shape flag (`--svg-shape`)

| Value               | Layout                             |
| ------------------- | ---------------------------------- |
| `strip`             | Classic color strip (default)      |
| `waveform`          | Gradient waveform path             |
| `split-stacked`     | Stacked per-band rectangles        |
| `split-waveform`    | Per-band waveform paths            |
| `split-lanes`       | Horizontal band lanes              |
| `split-centrifugal` | Centrifugal split layout           |
| `split-overlapping` | Overlapping semi-transparent bands |

Applies to both SVG and PNG output.

## Advanced options

Common tuning flags: `--normalize-mode`, `--deterministic-floor`, `--detection-mode`, `--frames-per-color`, `--band-edges-hz`, `--playback-rate`.

```bash
moodbar generate --help
moodbar batch --help
```

## Batch mode

```bash
moodbar batch -i ./music -o ./moods --progress
```

## Related crates

- [`moodbar-analysis`](https://crates.io/crates/moodbar-analysis) — DSP and rendering (source of truth)
- [`moodbar-decode`](https://crates.io/crates/moodbar-decode) — Symphonia decode → `analyze_pcm_mono`
