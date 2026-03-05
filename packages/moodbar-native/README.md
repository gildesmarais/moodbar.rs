# @moodbar/native

React Native bindings for Moodbar analysis and rendering (PNG/SVG) on iOS and Android.

## Install

```bash
npm install @moodbar/native
```

## Usage

```ts
import { analyze, render, generate } from "@moodbar/native";

const analysis = await analyze({ uri: "file:///path/to/input.mp3" });
const pngBytes = await render(analysis, "png", { width: 1200, height: 96 });
const svg = await render(analysis, "svg", { width: 1200, height: 96, shape: "Waveform" });

await analysis.dispose();

const fromMemory = await generate(
  { bytes: new Uint8Array([/* encoded audio bytes */]) },
  "png"
);
```

`analyze({ uri })` accepts:
- iOS: file paths and `file://` URIs
- Android: file paths, `file://` URIs, and `content://` URIs
