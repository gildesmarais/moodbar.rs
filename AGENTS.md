# Repository Guidelines

## Purpose and Founding Decisions
This Rust workspace is a CLI-first rewrite of Moodbar with two explicit goals:
1. Keep practical compatibility with legacy raw moodbar output (`R G B ...` bytes).
2. Evolve beyond legacy constraints with extensible analysis and rendering.

Core design choices:
- Analysis-first architecture: decode/analyze once, then render to multiple formats.
- Workspace split:
  - `crates/moodbar-core`: decoding, DSP, normalization, rendering primitives.
  - `crates/moodbar-cli`: user contract, subcommands, and automation-friendly output.
- Pure Rust audio stack (`symphonia` + `rustfft`) for cross-platform portability.

## Project Structure
- `crates/moodbar-core/src/lib.rs`: analysis pipeline, format renderers, tests.
- `crates/moodbar-core/tests/legacy_parity.rs`: fixture-based parity harness.
- `crates/moodbar-cli/src/main.rs`: `generate`, `batch`, `inspect` commands.
- `crates/moodbar-cli/tests/svg_golden.rs`: CLI integration/golden tests.
- `scripts/`: helper tooling (fixture generation, TDD helpers).
- `docs/plans/engineering-brief.md`: rationale and roadmap context.

## Algorithm and Performance Principles
- Streaming decode into frame analysis (avoid whole-track sample buffering).
- Reuse FFT/frame scratch buffers in hot paths (avoid per-frame allocations).
- Precompute FFT-bin-to-band mapping once per run.
- Keep deterministic controls explicit (`normalize_mode`, `deterministic_floor`).
- For SVG, cap gradient stop count (default bounded) while preserving analysis precision.

## Build, Test, and TDD Commands
- `make test-core`: fastest loop for DSP/core changes.
- `make test`: workspace tests.
- `make parity`: legacy parity harness.
- `make check`: required gate (`fmt`, `clippy -D warnings`, `test`).
- `make tdd-core` / `scripts/tdd-loop.sh -p moodbar-core`: watch-mode loops.

## Coding Style and Quality Bar
- Rust 2021, idiomatic ownership, explicit error propagation (`anyhow`/`thiserror`).
- Prefer small, composable functions and data-oriented structs.
- No unchecked performance regressions in hot loops; profile-sensitive code should avoid hidden allocations.
- New behavior must ship with tests (unit + integration when CLI contract is touched).

## Commit and PR Expectations
- Use Conventional Commits with intent/rationale (not file-by-file narration).
- Keep commits single-purpose; performance changes and behavior changes should be isolated.
- Before commit: run `make check`.
- If scope is cross-cutting, state why and define rollback boundary in commit body.

## Notes for Future Work
- Legacy fixture generation may be environment-constrained; keep parity tests tolerant to missing fixtures.
- Additive format work (SVG variants, future image outputs) should reuse `MoodbarAnalysis` rather than duplicating DSP logic.
