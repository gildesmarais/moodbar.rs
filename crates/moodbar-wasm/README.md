# @moodbar/wasm

WebAssembly bindings for Moodbar analysis and SVG rendering.

## Install

```bash
npm install @moodbar/wasm
```

## Usage

```js
import init, { analyze, svg } from "@moodbar/wasm";

await init();

const sampleRate = 44_100;
const pcm = new Float32Array(sampleRate).fill(0); // 1 second of silence
const analysis = analyze(pcm, sampleRate);
const svgMarkup = svg(analysis, { width: 600, height: 64, shape: "Strip" });
```
