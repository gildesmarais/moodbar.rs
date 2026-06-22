# @moodbar/wasm

WebAssembly bindings for moodbar analysis and rendering.

**No audio decode** — the browser (or host) decodes audio to mono `Float32Array` PCM. Calls `moodbar-analysis` directly; options JSON is parsed via `moodbar-bindings-schema`.

## Install

```bash
npm install @moodbar/wasm
```

## Usage

```js
import init, {
  analyze,
  analyze_with_options,
  svg,
  png,
  raw_rgb,
} from "@moodbar/wasm";

await init();

const sampleRate = 44_100;
const pcm = new Float32Array(sampleRate).fill(0);

const analysis = analyze(pcm, sampleRate);

// or with JSON options (GenerateOptionsPatch fields)
const tuned = analyze_with_options(pcm, sampleRate, {
  normalize_mode: "GlobalPeak",
  max_target_frames: 1500,
});

const svgMarkup = svg(analysis, {
  width: 600,
  height: 64,
  shape: "SplitStacked",
});

const pngBytes = png(analysis, { width: 600, height: 64, shape: "SplitStacked" });

const legacyBytes = raw_rgb(analysis);
```

## Svg `shape` values

`Strip`, `Waveform`, `SplitStacked`, `SplitWaveform`, `SplitLanes`, `SplitCentrifugal`, `SplitOverlapping` (PascalCase in JSON options).

Split SVG output includes CSS classes (`mood-low`, `mood-mid`, `mood-high`) for styling.

## Related crates

- [`moodbar-analysis`](https://crates.io/crates/moodbar-analysis) — Rust DSP/render core
- [`moodbar-bindings-schema`](https://crates.io/crates/moodbar-bindings-schema) — option patch types
