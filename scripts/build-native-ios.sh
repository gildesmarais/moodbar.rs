#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
CRATE_NAME="moodbar-native-ffi"
CRATE_LIB="libmoodbar_native_ffi.a"
OUT_DIR="$ROOT_DIR/packages/moodbar-native/ios"
HEADER_SRC="$ROOT_DIR/crates/moodbar-native-ffi/include/moodbar_native_ffi.h"
BUILD_PROFILE="mobile-release"

TARGETS=(
  "aarch64-apple-ios"
  "aarch64-apple-ios-sim"
  "x86_64-apple-ios"
)

for target in "${TARGETS[@]}"; do
  rustup target add "$target"
  cargo build -p "$CRATE_NAME" --profile "$BUILD_PROFILE" --target "$target"
done

mkdir -p "$OUT_DIR/include"
cp "$HEADER_SRC" "$OUT_DIR/include/moodbar_native_ffi.h"

rm -rf "$OUT_DIR/MoodbarNativeFFI.xcframework"

xcodebuild -create-xcframework \
  -library "$ROOT_DIR/target/aarch64-apple-ios/$BUILD_PROFILE/$CRATE_LIB" -headers "$OUT_DIR/include" \
  -library "$ROOT_DIR/target/aarch64-apple-ios-sim/$BUILD_PROFILE/$CRATE_LIB" -headers "$OUT_DIR/include" \
  -library "$ROOT_DIR/target/x86_64-apple-ios/$BUILD_PROFILE/$CRATE_LIB" -headers "$OUT_DIR/include" \
  -output "$OUT_DIR/MoodbarNativeFFI.xcframework"
