#!/usr/bin/env bash

# Exit immediately if a command exits with a non-zero status
set -e

# Default input file path
DEFAULT_INPUT="rick.wav"
INPUT="${1:-$DEFAULT_INPUT}"

# Check if input file exists
if [ ! -f "$INPUT" ]; then
  echo "Error: Input audio file not found at: $INPUT"
  echo "Usage: bash generate-rick-moodbars.sh [path/to/audio/file]"
  exit 1
fi

echo "=================================================="
echo "Building moodbar-cli in release mode..."
echo "=================================================="
cargo build --release -p moodbar

# Output directory
OUT_DIR="docs/assets/rick"
mkdir -p "$OUT_DIR"

echo ""
echo "=================================================="
echo "Generating Rick Astley Moodbars..."
echo "Input: $INPUT"
echo "Output Directory: $OUT_DIR"
echo "=================================================="

# 1. Classic Waveform PNG
echo "Generating: 1. Waveform [Classic Theme] -> $OUT_DIR/classic-waveform.png"
./target/release/moodbar generate \
  -i "$INPUT" \
  --format png \
  --svg-shape waveform \
  --theme classic \
  --output "$OUT_DIR/classic-waveform.png" \
  --force

# 2. Split Stacked PNG
echo "Generating: 2. Split Stacked [Classic Theme] -> $OUT_DIR/classic-split-stacked.png"
./target/release/moodbar generate \
  -i "$INPUT" \
  --format png \
  --svg-shape split-stacked \
  --theme classic \
  --output "$OUT_DIR/classic-split-stacked.png" \
  --force

# 3. Split Lanes PNG (Cool Theme)
echo "Generating: 3. Split Lanes [Cool Theme] -> $OUT_DIR/cool-split-lanes.png"
./target/release/moodbar generate \
  -i "$INPUT" \
  --format png \
  --svg-shape split-lanes \
  --theme cool \
  --output "$OUT_DIR/cool-split-lanes.png" \
  --force

# 4. Split Centrifugal PNG (Light Theme)
echo "Generating: 4. Split Centrifugal [Light Theme] -> $OUT_DIR/light-split-centrifugal.png"
./target/release/moodbar generate \
  -i "$INPUT" \
  --format png \
  --svg-shape split-centrifugal \
  --theme light \
  --output "$OUT_DIR/light-split-centrifugal.png" \
  --force

# 5. Split Overlapping PNG (Custom Neon Colors)
echo "Generating: 5. Split Overlapping [Custom Colors] -> $OUT_DIR/custom-split-overlapping.png"
./target/release/moodbar generate \
  -i "$INPUT" \
  --format png \
  --svg-shape split-overlapping \
  --colors "#e91e63,#00ffcc,#ffeb3b" \
  --output "$OUT_DIR/custom-split-overlapping.png" \
  --force

# 6. Classic Strip PNG
echo "Generating: 6. Classic Strip [Classic Theme] -> $OUT_DIR/classic-strip.png"
./target/release/moodbar generate \
  -i "$INPUT" \
  --format png \
  --svg-shape strip \
  --theme classic \
  --output "$OUT_DIR/classic-strip.png" \
  --force

# 7. Split Waveform PNG
echo "Generating: 7. Split Waveform PNG [Classic Theme] -> $OUT_DIR/classic-split-waveform.png"
./target/release/moodbar generate \
  -i "$INPUT" \
  --format png \
  --svg-shape split-waveform \
  --theme classic \
  --output "$OUT_DIR/classic-split-waveform.png" \
  --force

echo ""
echo "=================================================="
echo "Successfully generated all moodbar assets!"
echo "Files are saved in: $OUT_DIR"
echo "=================================================="
