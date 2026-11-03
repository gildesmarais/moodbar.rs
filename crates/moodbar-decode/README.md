# moodbar-decode

[![Crates.io](https://img.shields.io/crates/v/moodbar-decode.svg)](https://crates.io/crates/moodbar-decode)

Symphonia-based audio decode for the moodbar pipeline.

Decodes an audio file or in-memory bytes packet by packet, downmixes each packet to mono and streams it into `moodbar_analysis::FrameAnalyzer`; the full PCM is never buffered. Stereo is averaged (NEON on aarch64); streams with more than two channels average the first two channels. Decode diagnostics (`decode_errors`, `zero_channel_packets`, `truncated_frames`) are set on the returned `MoodbarAnalysis`.

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
