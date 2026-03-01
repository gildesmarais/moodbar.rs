.PHONY: help test test-core test-cli parity fmt lint check tdd tdd-core ci

help:
	@echo "Targets:"
	@echo "  make test       - run all tests"
	@echo "  make test-core  - run moodbar-core tests"
	@echo "  make parity     - run legacy parity test"
	@echo "  make fmt        - check formatting"
	@echo "  make lint       - run clippy with warnings as errors"
	@echo "  make check      - fmt + lint + test"
	@echo "  make tdd        - watch all tests (cargo-watch if available)"
	@echo "  make tdd-core   - watch moodbar-core tests"
	@echo "  make ci         - local CI gate"

test:
	cargo test --workspace

test-core:
	cargo test -p moodbar-core

test-cli:
	cargo test -p moodbar

parity:
	cargo test -p moodbar-core --test legacy_parity

fmt:
	cargo fmt --all -- --check

lint:
	cargo clippy --workspace --all-targets -- -D warnings

check: fmt lint test

ci: check

# TDD loop: automatically reruns tests when files change if cargo-watch is installed.
tdd:
	@if command -v cargo-watch >/dev/null 2>&1; then \
		cargo watch -q -w crates -w tests -x "test --workspace"; \
	else \
		echo "cargo-watch not found; running one test pass instead"; \
		cargo test --workspace; \
	fi

tdd-core:
	@if command -v cargo-watch >/dev/null 2>&1; then \
		cargo watch -q -w crates/moodbar-core -w tests -x "test -p moodbar-core"; \
	else \
		echo "cargo-watch not found; running one test pass instead"; \
		cargo test -p moodbar-core; \
	fi
