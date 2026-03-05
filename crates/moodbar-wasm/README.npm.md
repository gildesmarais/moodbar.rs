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

const analysis = analyze(new Uint8Array([0, 64, 128, 255]));
const svgMarkup = svg(analysis, 600, 64);
```
