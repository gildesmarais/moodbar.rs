#!/usr/bin/env python3
"""Generate legacy parity fixtures for rust/tests/fixtures/legacy.

Usage example:
  python3 rust/scripts/generate_legacy_fixture.py \
    --name tri_band \
    --legacy-bin ./build/moodbar
"""

from __future__ import annotations

import argparse
import json
import math
import pathlib
import struct
import subprocess
import wave


SAMPLE_RATE = 44_100
SEGMENT_SECONDS = 0.6


def sine_segment(freq: float, amp: float, duration_s: float) -> list[float]:
    n = int(SAMPLE_RATE * duration_s)
    out = []
    for i in range(n):
        t = i / SAMPLE_RATE
        out.append(amp * math.sin(2.0 * math.pi * freq * t))
    return out


def write_wav(path: pathlib.Path, samples: list[float]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(SAMPLE_RATE)
        pcm = bytearray()
        for x in samples:
            x = max(-1.0, min(1.0, x))
            pcm += struct.pack("<h", int(x * 32767.0))
        w.writeframes(bytes(pcm))


def run(cmd: list[str]) -> None:
    subprocess.run(cmd, check=True)


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser()
    p.add_argument("--name", required=True, help="Fixture base name")
    p.add_argument("--legacy-bin", required=True, help="Path to legacy moodbar executable")
    p.add_argument(
        "--fixtures-dir",
        default="rust/tests/fixtures/legacy",
        help="Fixture output directory",
    )
    p.add_argument("--fft-size", type=int, default=2048)
    p.add_argument("--low-cut-hz", type=float, default=500.0)
    p.add_argument("--mid-cut-hz", type=float, default=2000.0)
    p.add_argument(
        "--normalize-mode",
        choices=["per_channel_peak", "global_peak"],
        default="per_channel_peak",
    )
    p.add_argument("--deterministic-floor", type=float, default=1e-12)
    p.add_argument("--max-mean-abs-diff", type=float, default=5.0)
    p.add_argument("--max-abs-diff", type=int, default=32)
    return p.parse_args()


def main() -> None:
    args = parse_args()
    root = pathlib.Path(args.fixtures_dir)
    root.mkdir(parents=True, exist_ok=True)

    wav_path = root / f"{args.name}.wav"
    legacy_mood = root / f"{args.name}.legacy.mood"
    manifest = root / f"{args.name}.json"

    samples: list[float] = []
    samples.extend(sine_segment(100.0, 0.45, SEGMENT_SECONDS))
    samples.extend(sine_segment(1000.0, 0.30, SEGMENT_SECONDS))
    samples.extend(sine_segment(5000.0, 0.18, SEGMENT_SECONDS))
    write_wav(wav_path, samples)

    run([args.legacy_bin, "-o", str(legacy_mood), str(wav_path)])

    payload = {
        "name": args.name,
        "input_audio": wav_path.name,
        "expected_mood": legacy_mood.name,
        "options": {
            "fft_size": args.fft_size,
            "low_cut_hz": args.low_cut_hz,
            "mid_cut_hz": args.mid_cut_hz,
            "normalize_mode": args.normalize_mode,
            "deterministic_floor": args.deterministic_floor,
        },
        "max_mean_abs_diff": args.max_mean_abs_diff,
        "max_abs_diff": args.max_abs_diff,
    }
    manifest.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")

    print(f"generated fixture: {manifest}")


if __name__ == "__main__":
    main()
