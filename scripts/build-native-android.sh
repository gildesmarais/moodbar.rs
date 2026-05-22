#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
OUT_DIR="$ROOT_DIR/packages/moodbar-native/android/src/main/jniLibs"
HEADER_SRC="$ROOT_DIR/crates/moodbar-native-ffi/include/moodbar_native_ffi.h"
BUILD_PROFILE="mobile-release"

if ! command -v cargo-ndk >/dev/null 2>&1; then
  echo "cargo-ndk is required. Install with: cargo install cargo-ndk"
  exit 1
fi

mkdir -p "$OUT_DIR" "$ROOT_DIR/packages/moodbar-native/android/src/main/cpp/include"
cp "$HEADER_SRC" "$ROOT_DIR/packages/moodbar-native/android/src/main/cpp/include/moodbar_native_ffi.h"

cargo ndk \
  --target arm64-v8a \
  --target armeabi-v7a \
  --target x86_64 \
  --target x86 \
  --platform 24 \
  -o "$OUT_DIR" \
  build -p moodbar-native-ffi --profile "$BUILD_PROFILE"
