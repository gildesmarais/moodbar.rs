#!/usr/bin/env bash
set -euo pipefail

# Usage:
#   scripts/tdd-loop.sh                 # workspace tests
#   scripts/tdd-loop.sh -p moodbar-core # specific package
#   scripts/tdd-loop.sh -- <args>       # raw cargo test args

args=("test" "--workspace")
if [[ $# -gt 0 ]]; then
  args=("test" "$@")
fi

if command -v cargo-watch >/dev/null 2>&1; then
  cargo watch -q -w crates -w tests -x "${args[*]}"
else
  echo "cargo-watch not found; running single pass: cargo ${args[*]}"
  cargo "${args[@]}"
fi
