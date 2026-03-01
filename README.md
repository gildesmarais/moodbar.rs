# Rust Rewrite (`rust/`)

This workspace hosts the Rust rewrite of Moodbar with a CLI-first, cross-platform design.

## Crates
- `crates/moodbar-core`: audio decode + spectral analysis + `.mood` byte generation.
- `crates/moodbar-cli` (binary name `moodbar`): user-facing commands.

## Current CLI (modern contract)
```bash
cargo run -p moodbar -- generate -i input.ogg -o output.mood
cargo run -p moodbar -- generate -i input.ogg -o output.svg --format svg --svg-shape waveform
cargo run -p moodbar -- batch -i ./music -o ./moods
cargo run -p moodbar -- inspect -i output.mood
```

Use `--json` for machine-readable results.
Use `--normalize-mode` and `--deterministic-floor` when tuning deterministic output behavior.
Use `--detection-mode spectral-flux`, `--frames-per-color 1000`, and `--band-edges-hz 200,600,1200,2400` for algorithm variants.

## TDD Workflow
Red/Green/Refactor loop:
1. Write or update a test first (unit test in `crates/*/src/*.rs` or integration test in `crates/*/tests/`).
2. Run the smallest target that should fail, then implement the fix.
3. Refactor with tests green, then run full gate.

Common commands:
```bash
make test-core            # fastest loop for core logic
make parity               # legacy compatibility check
make test                 # whole workspace
make check                # fmt + clippy + tests
make tdd-core             # auto-rerun core tests on file changes (if cargo-watch installed)
scripts/tdd-loop.sh -p moodbar-core
```

## Engineering Constraints from Discovery
- v1 scope: single-file + batch generation.
- Compatibility target: preserve existing raw moodbar format semantics (`R G B ...` bytes).
- Platform target: Linux/macOS required, Windows native where possible.
- Architecture target: workspace split (`moodbar-core` + `moodbar-cli`).
- Delivery strategy: production-grade v1 before broad rollout.

## Next Steps
1. Generate legacy fixtures:
   `python3 rust/scripts/generate_legacy_fixture.py --name tri_band --legacy-bin ./build/moodbar`
2. Run parity tests:
   `cargo test -p moodbar-core --test legacy_parity`
   Note: this test is designed to skip when no legacy fixture manifests are present.
3. Run local quality gate:
   `make check`
4. Promote Linux CI workflow from `rust/.github/workflows/linux-rust-ci.yml` to repo root `.github/workflows/` when Rust becomes the top-level CI target.
