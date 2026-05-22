# Web WASM Example

Minimal browser usage for `@moodbar/wasm`.

## Install

```bash
npm install @moodbar/wasm
```

## Example

```js
import init, { analyze, svg } from "@moodbar/wasm";

await init();

const sampleRate = 44_100;
const pcm = new Float32Array(sampleRate).fill(0);
const analysis = analyze(pcm, sampleRate);
const markup = svg(analysis, { width: 800, height: 64, shape: "Waveform" });

document.getElementById("root").innerHTML = markup;
```
