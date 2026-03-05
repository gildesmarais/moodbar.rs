.PHONY: help test test-core test-cli parity fmt lint check tdd tdd-core ci wasm publish-check-wasm wasm-docs

WASM_NPM_PACKAGE_DIR := crates/moodbar-wasm/pkg
WASM_NPM_TEMPLATE := crates/moodbar-wasm/package.npm.json
WASM_NPM_README := crates/moodbar-wasm/README.npm.md
NPM_CACHE_DIR ?= .npm-cache

help:
	@echo "Targets:"
	@echo "  make test       - run all tests"
	@echo "  make test-core  - run moodbar-core tests"
	@echo "  make parity     - run legacy parity test"
	@echo "  make wasm       - build the wasm package (requires wasm-pack)"
	@echo "  make publish-check-wasm - build and validate npm package contents"
	@echo "  make wasm-docs  - build wasm assets for GitHub Pages under docs/assets/"
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

wasm:
	wasm-pack build crates/moodbar-wasm --release --target bundler
	node scripts/prepare-npm-package.mjs \
		--package-dir $(WASM_NPM_PACKAGE_DIR) \
		--template $(WASM_NPM_TEMPLATE) \
		--readme $(WASM_NPM_README)

publish-check-wasm: wasm
	node scripts/verify-npm-package.mjs \
		--package-dir $(WASM_NPM_PACKAGE_DIR) \
		--expected-name @moodbar/wasm \
		--required-files README.md,LICENSE-MIT,LICENSE-APACHE,moodbar_wasm.js,moodbar_wasm_bg.wasm,package.json
	npm pack ./$(WASM_NPM_PACKAGE_DIR) --dry-run --json --cache $(NPM_CACHE_DIR)

wasm-docs:
	wasm-pack build crates/moodbar-wasm --release --target web --out-dir ../../docs/assets/moodbar-wasm

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
