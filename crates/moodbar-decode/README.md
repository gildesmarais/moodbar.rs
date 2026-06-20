# moodbar-decode

[![Crates.io](https://img.shields.io/crates/v/moodbar-decode.svg)](https://crates.io/crates/moodbar-decode)

Symphonia-based audio decode for the moodbar pipeline.

Decodes an audio file or in-memory bytes to mono PCM, then calls `moodbar_analysis::analyze_pcm_mono`. Decode diagnostics (`decode_errors`, `zero_channel_packets`, `truncated_frames`) are copied onto the returned `MoodbarAnalysis`.

Omit this crate when the host already provides PCM (browser Web Audio API, platform decoders).

## Usage

```rust
use moodbar_decode::analyze_path;
use moodbar_analysis::GenerateOptions;
use std::path::Path;

let path = Path::new("test.mp3");
let options = GenerateOptions::default();

match analyze_path(path, &options) {
    Ok(analysis) => {
        println!("Generated {} frames.", analysis.frames.len());
    }
    Err(e) => eprintln!("Failed: {e}"),
}
```

Convenience helpers `generate_moodbar_from_path` and `generate_moodbar_from_bytes` return legacy raw RGB bytes.

## Supported formats

All formats enabled by default in Symphonia: `mp3`, `ogg` (Vorbis), `flac`, `wav`, `aac`, `mp4`, and others Symphonia probes.

## Note on buffering

The decode path collects the full mono PCM buffer before analysis. Streaming handoff into `FrameAnalyzer` is a future optimization; analysis itself supports chunked `feed_mono_samples` when driven directly via `moodbar-analysis`.
