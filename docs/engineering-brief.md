# Moodbar Rust Rewrite Engineering Brief

## Decision Summary
- `S1A+S1B`: v1 supports single-file and batch generation.
- `S2A`: target byte-compatible moodbar output where feasible.
- `C1C`: modern CLI with subcommands.
- `C2B`: batch continues on per-file failures and returns nonzero exit if any fail.
- `C3B`: human output plus `--json` mode.
- `P1A/P1B`: Linux + macOS are hard requirements; Windows native is best effort.
- `P2B`: x86_64 + aarch64.
- `P3A`: distribute via `cargo install` initially.
- `A1B`: pure-Rust audio stack (Symphonia + RustFFT).
- `A2B` aiming for `A2A`: best-effort determinism now, design for stronger determinism later.
- `A3B`: balanced correctness and throughput.
- `D1B`: workspace with `moodbar-core` and `moodbar-cli`.
- `D2B`: production-grade v1 target.
- `D3A`: Linux CI first.

## Proposed Architecture
- `moodbar-core`
  - Decode audio to mono PCM (`f32`) with Symphonia.
  - Analyze windowed FFT frames with RustFFT.
  - Map low/mid/high bands to RGB and normalize into raw moodbar bytes.
  - Expose stable API that can support future output variants.
- `moodbar-cli`
  - Commands: `generate`, `batch`, `inspect`.
  - Shared options for FFT size and band cutoffs.
  - Optional JSON output for automation.

## Risks
- Exact parity with legacy C++/GStreamer may differ due to decoder and floating-point differences.
- Determinism across platforms may require tighter numeric controls and golden fixtures.

## Milestones
1. Parity baseline: fixture corpus + comparison harness.
2. Determinism hardening: numeric normalization and tolerance policy.
3. Performance pass: streaming decode path and memory optimizations.
4. Platform hardening: expand CI matrix and release automation.
