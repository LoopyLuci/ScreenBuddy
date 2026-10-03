"""Verify the generated sounds are real audio rather than silence or noise.

Checks each WAV for the properties that would make it useless in practice:
non-trivial amplitude, no long stretches of silence, no clipping, and a peak
that is not loud enough to startle. Deliberately checks content, not just that
the files exist.
"""

from __future__ import annotations

import math
import struct
import sys
import wave
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RAW = ROOT / "ScreenBuddy-Android" / "app" / "src" / "main" / "res" / "raw"

EXPECTED = {
    "celebrate": (0.5, 1.2),   # (min seconds, max seconds)
    "step": (0.03, 0.2),
    "flutter": (0.05, 0.3),
    "snore": (0.6, 1.5),
    "click": (0.01, 0.1),
}


def analyse(path: Path) -> dict:
    with wave.open(str(path), "rb") as handle:
        channels = handle.getnchannels()
        width = handle.getsampwidth()
        rate = handle.getframerate()
        frames = handle.readframes(handle.getnframes())

    if width != 2:
        raise ValueError(f"{path.name}: expected 16-bit, got {width * 8}-bit")

    samples = struct.unpack(f"<{len(frames) // 2}h", frames)
    peak = max(abs(s) for s in samples) / 32768.0
    rms = math.sqrt(sum(s * s for s in samples) / len(samples)) / 32768.0
    clipped = sum(1 for s in samples if abs(s) >= 32700)

    # Longest run of consecutive near-silent samples, as a fraction of length.
    longest = 0
    current = 0
    for s in samples:
        if abs(s) < 40:  # roughly -58 dBFS
            current += 1
            longest = max(longest, current)
        else:
            current = 0
    silence_ratio = longest / len(samples)

    return {
        "channels": channels,
        "rate": rate,
        "seconds": len(samples) / rate,
        "peak": peak,
        "rms": rms,
        "clipped": clipped,
        "silence_ratio": silence_ratio,
    }


def main() -> int:
    failures = 0
    print(f"{'file':<14}{'dur':>7}{'peak':>8}{'rms':>8}{'clip':>6}{'silence':>9}  verdict")

    for name, (low, high) in EXPECTED.items():
        path = RAW / f"{name}.wav"
        if not path.exists():
            print(f"{name:<14}{'-':>7}{'-':>8}{'-':>8}{'-':>6}{'-':>9}  MISSING")
            failures += 1
            continue

        info = analyse(path)
        problems = []
        if not (low <= info["seconds"] <= high):
            problems.append(f"duration {info['seconds']:.2f}s outside {low}-{high}")
        if info["peak"] < 0.03:
            problems.append(f"peak {info['peak']:.4f} is effectively silent")
        if info["peak"] > 0.9:
            problems.append(f"peak {info['peak']:.3f} is loud enough to startle")
        if info["rms"] < 0.005:
            problems.append(f"rms {info['rms']:.5f} is too quiet to hear")
        if info["clipped"]:
            problems.append(f"{info['clipped']} clipped samples")
        if info["silence_ratio"] > 0.5:
            problems.append(f"{info['silence_ratio']:.0%} of it is silence")
        if info["channels"] != 1:
            problems.append(f"{info['channels']} channels")

        verdict = "ok" if not problems else "FAIL: " + "; ".join(problems)
        if problems:
            failures += 1
        print(
            f"{name:<14}{info['seconds']:>6.2f}s{info['peak']:>8.3f}"
            f"{info['rms']:>8.4f}{info['clipped']:>6d}"
            f"{info['silence_ratio']:>9.0%}  {verdict}"
        )

    print(f"\n{len(EXPECTED) - failures}/{len(EXPECTED)} sounds are usable")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())