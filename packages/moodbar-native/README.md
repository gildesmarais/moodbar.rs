# @moodbar/native

React Native / Expo bindings for moodbar on iOS and Android.

Decode and analysis run in Rust (`moodbar-decode` + `moodbar-analysis` via `moodbar-native-ffi`). Rendering uses the same `SvgShape` variants as the CLI and WASM packages.

## Install

```bash
npm install @moodbar/native
```

## Usage

```ts
import { analyze, render, generate } from "@moodbar/native";

const analysis = await analyze({ uri: "file:///path/to/input.mp3" });

const pngBytes = await render(analysis, "png", {
  width: 1200,
  height: 96,
  shape: "SplitStacked",
});

const svg = await render(analysis, "svg", {
  width: 1200,
  height: 96,
  shape: "SplitWaveform",
});

await analysis.dispose();

const fromMemory = await generate(
  { bytes: new Uint8Array([/* encoded audio */]) },
  "png",
  { shape: "Waveform" }
);
```

## Input URIs (`analyze({ uri })`)

- **iOS:** file paths and `file://` URIs
- **Android:** file paths, `file://`, and `content://` URIs

## SvgShape values

`Strip` | `Waveform` | `SplitStacked` | `SplitWaveform` | `SplitLanes` | `SplitCentrifugal` | `SplitOverlapping`

Split SVG output exposes CSS classes `mood-low`, `mood-mid`, `mood-high`.

## Options

Analysis options match `AnalyzeOptions` in `index.d.ts` (FFT, normalization, band edges, `max_target_frames`, `playback_rate`, etc.). Render options: `width`, `height`, `shape`, `background`, `max_gradient_stops`.
