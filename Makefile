.PHONY: help test test-core test-cli parity fmt lint check tdd tdd-core ci wasm publish-check-wasm wasm-docs native native-ios native-android publish-check-native publish-check-native-ios publish-check-native-android

WASM_NPM_PACKAGE_DIR := crates/moodbar-wasm/pkg
WASM_NPM_PACKAGE_JSON_SOURCE := crates/moodbar-wasm/package.json
WASM_NPM_README_SOURCE := crates/moodbar-wasm/README.md
WASM_NPM_REPOSITORY_URL := git+https://github.com/gildesmarais/moodbar.rs.git
WASM_DEMO_PACKAGE_DIR := packages/moodbar-wasm
NATIVE_NPM_PACKAGE_DIR := packages/moodbar-native
NATIVE_NPM_PACKAGE_JSON_SOURCE := packages/moodbar-native/package.json
NATIVE_NPM_README_SOURCE := packages/moodbar-native/README.md
NATIVE_NPM_REPOSITORY_URL := git+https://github.com/gildesmarais/moodbar.rs.git
NPM_CACHE_DIR ?= .npm-cache

help:
	@echo "Targets:"
	@echo "  make test       - run all tests"
	@echo "  make test-core  - run moodbar-core tests"
	@echo "  make parity     - run legacy parity test"
	@echo "  make wasm       - build the wasm package (requires wasm-pack and node)"
	@echo "  make publish-check-wasm - build and validate npm package contents"
	@echo "  make native     - prepare @moodbar/native package metadata/files"
	@echo "  make native-ios - build iOS xcframework for @moodbar/native"
	@echo "  make native-android - build Android JNI libs for @moodbar/native"
	@echo "  make publish-check-native - validate native npm package contents"
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
	node scripts/prepare-package.mjs \
		--package-dir $(WASM_NPM_PACKAGE_DIR) \
		--package-json-source $(WASM_NPM_PACKAGE_JSON_SOURCE) \
		--readme-source $(WASM_NPM_README_SOURCE)
	rm -rf $(WASM_DEMO_PACKAGE_DIR)
	mkdir -p $(WASM_DEMO_PACKAGE_DIR)
	cp -R $(WASM_NPM_PACKAGE_DIR)/. $(WASM_DEMO_PACKAGE_DIR)/

publish-check-wasm: wasm
	node scripts/verify-npm-package.mjs \
		--package-dir $(WASM_NPM_PACKAGE_DIR) \
		--expected-name @moodbar/wasm \
		--expected-repository-url $(WASM_NPM_REPOSITORY_URL) \
		--required-files README.md,LICENSE-MIT,LICENSE-APACHE,moodbar_wasm.js,moodbar_wasm_bg.wasm,package.json
	npm pack ./$(WASM_NPM_PACKAGE_DIR) --dry-run --json --cache $(NPM_CACHE_DIR)

wasm-docs:
	wasm-pack build crates/moodbar-wasm --release --target web --out-dir ../../docs/assets/moodbar-wasm

native:
	node scripts/prepare-package.mjs \
		--package-dir $(NATIVE_NPM_PACKAGE_DIR) \
		--package-json-source $(NATIVE_NPM_PACKAGE_JSON_SOURCE) \
		--readme-source $(NATIVE_NPM_README_SOURCE)

native-ios:
	./scripts/build-native-ios.sh
	$(MAKE) native

native-android:
	./scripts/build-native-android.sh
	$(MAKE) native

publish-check-native: native-ios native-android
	node scripts/verify-npm-package.mjs \
		--package-dir $(NATIVE_NPM_PACKAGE_DIR) \
		--expected-name @moodbar/native \
		--expected-repository-url $(NATIVE_NPM_REPOSITORY_URL) \
		--required-files README.md,LICENSE-MIT,LICENSE-APACHE,index.js,index.d.ts,expo-module.config.json,moodbar-native.podspec,package.json,ios/MoodbarNativeFFI.xcframework/ios-arm64/libmoodbar_native_ffi.a,ios/MoodbarNativeFFI.xcframework/ios-arm64_x86_64-simulator/libmoodbar_native_ffi_simulator.a,android/src/main/jniLibs/arm64-v8a/libmoodbar_native_ffi.so,android/src/main/jniLibs/armeabi-v7a/libmoodbar_native_ffi.so,android/src/main/jniLibs/x86/libmoodbar_native_ffi.so,android/src/main/jniLibs/x86_64/libmoodbar_native_ffi.so
	npm pack ./$(NATIVE_NPM_PACKAGE_DIR) --dry-run --json --cache $(NPM_CACHE_DIR)

publish-check-native-ios: native-ios
	node scripts/verify-npm-package.mjs \
		--package-dir $(NATIVE_NPM_PACKAGE_DIR) \
		--expected-name @moodbar/native \
		--expected-repository-url $(NATIVE_NPM_REPOSITORY_URL) \
		--required-files README.md,LICENSE-MIT,LICENSE-APACHE,index.js,index.d.ts,expo-module.config.json,moodbar-native.podspec,package.json,ios/MoodbarNativeFFI.xcframework/ios-arm64/libmoodbar_native_ffi.a,ios/MoodbarNativeFFI.xcframework/ios-arm64_x86_64-simulator/libmoodbar_native_ffi_simulator.a
	npm pack ./$(NATIVE_NPM_PACKAGE_DIR) --dry-run --json --cache $(NPM_CACHE_DIR)

publish-check-native-android: native-android
	node scripts/verify-npm-package.mjs \
		--package-dir $(NATIVE_NPM_PACKAGE_DIR) \
		--expected-name @moodbar/native \
		--expected-repository-url $(NATIVE_NPM_REPOSITORY_URL) \
		--required-files README.md,LICENSE-MIT,LICENSE-APACHE,index.js,index.d.ts,expo-module.config.json,moodbar-native.podspec,package.json,android/src/main/jniLibs/arm64-v8a/libmoodbar_native_ffi.so,android/src/main/jniLibs/armeabi-v7a/libmoodbar_native_ffi.so,android/src/main/jniLibs/x86/libmoodbar_native_ffi.so,android/src/main/jniLibs/x86_64/libmoodbar_native_ffi.so
	npm pack ./$(NATIVE_NPM_PACKAGE_DIR) --dry-run --json --cache $(NPM_CACHE_DIR)

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
