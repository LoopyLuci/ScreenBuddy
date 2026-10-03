"""Generate the creature and UI sound effects.

These are synthesised, not sampled: a short additive tone with an envelope
produces a clean, royalty-free sound with no binary blobs of unknown provenance
in the repository. Each one is deliberately tiny and quiet, because a desktop
companion that chirps is worse than one that does not.

Written as 16-bit mono PCM WAV at 22050 Hz, which SoundPool accepts directly.
Run from the repository root:

    python tools/generate_sounds.py

The output is committed, so this only needs re-running if a sound is redesigned.
"""

from __future__ import annotations

import math
import struct
import sys
import wave
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "ScreenBuddy-Android" / "app" / "src" / "main" / "res" / "raw"

RATE = 22050
AMPLITUDE = 0.18  # Deliberately quiet: this plays alongside work.


def envelope(index: int, total: int, attack: float = 0.02, release: float = 0.5) -> float:
    """A simple attack/decay shape, so nothing starts or ends with a click."""
    t = index / total
    if t < attack:
        return t / attack
    # Cosine decay from the end, which is smooth at both ends.
    remaining = (1.0 - t) / (1.0 - attack)
    return max(0.0, math.cos((1.0 - remaining) * math.pi / 2) ** 2 * remaining)


def tone(
    seconds: float,
    partials: list[tuple[float, float]],
    *,
    vibrato: float = 0.0,
    vibrato_hz: float = 0.0,
    attack: float = 0.02,
    release: float = 0.5,
) -> bytes:
    """Render a sum of sine partials to 16-bit mono PCM.

    partials is (frequency_hz, relative_amplitude). A single partial is a pure
    tone; two or three give it a timbre that reads as an object rather than a
    beep.
    """
    total = int(RATE * seconds)
    frames = bytearray()
    for i in range(total):
        t = i / RATE
        value = 0.0
        for frequency, weight in partials:
            f = frequency
            if vibrato:
                f *= 1.0 + vibrato * math.sin(2 * math.pi * vibrato_hz * t)
            value += weight * math.sin(2 * math.pi * f * t)
        value *= AMPLITUDE * envelope(i, total, attack, release)
        # Clamp rather than wrap: an overshoot must not invert the phase.
        value = max(-1.0, min(1.0, value))
        frames += struct.pack("<h", int(value * 32767))
    return bytes(frames)


def sweep(seconds: float, f_start: float, f_end: float, **kwargs) -> bytes:
    """A tone that glides between two frequencies."""
    total = int(RATE * seconds)
    frames = bytearray()
    phase = 0.0
    for i in range(total):
        progress = i / total
        frequency = f_start + (f_end - f_start) * progress
        phase += 2 * math.pi * frequency / RATE
        value = math.sin(phase) * AMPLITUDE * envelope(i, total, **kwargs)
        frames += struct.pack("<h", int(max(-1.0, min(1.0, value)) * 32767))
    return bytes(frames)


def noise_burst(seconds: float, seed: int = 7, decay: float = 18.0) -> bytes:
    """A short filtered noise transient, for a footstep.

    Deterministic rather than random so the committed file is reproducible.
    """
    total = int(RATE * seconds)
    state = seed
    frames = bytearray()
    previous = 0.0
    for i in range(total):
        # xorshift, so no dependency and identical output every run.
        state ^= (state << 13) & 0xFFFFFFFF
        state ^= state >> 17
        state ^= (state << 5) & 0xFFFFFFFF
        white = ((state & 0xFFFF) / 32768.0) - 1.0
        # One-pole low pass, to take the fizz off.
        previous = 0.7 * previous + 0.3 * white
        value = previous * AMPLITUDE * math.exp(-decay * i / RATE)
        frames += struct.pack("<h", int(max(-1.0, min(1.0, value)) * 32767))
    return bytes(frames)


def write(name: str, data: bytes) -> None:
    path = OUT / f"{name}.wav"
    with wave.open(str(path), "wb") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(RATE)
        handle.writeframes(data)
    print(f"  {path.name:<16} {len(data):>6} bytes  {len(data) / 2 / RATE:.2f}s")


def main() -> int:
    if not OUT.parent.exists():
        print(f"missing {OUT.parent}", file=sys.stderr)
        return 1
    OUT.mkdir(parents=True, exist_ok=True)

    # A rising two-note chime: unmistakably a celebration, still quiet.
    write(
        "celebrate",
        tone(0.42, [(784.0, 0.6), (1568.0, 0.25)])
        + tone(0.38, [(1046.5, 0.6), (2093.0, 0.2)], release=0.7),
    )
    # A soft footfall.
    write("step", noise_burst(0.07, seed=11, decay=42.0))
    # A wingbeat: two quick filtered noise taps.
    write(
        "flutter",
        noise_burst(0.05, seed=23, decay=52.0)
        + noise_burst(0.05, seed=29, decay=52.0),
    )
    # A slow breathy drone for sleep.
    write("snore", tone(0.95, [(98.0, 0.7), (196.0, 0.15)], release=0.8))
    # A dry UI tick.
    write("click", tone(0.035, [(1760.0, 0.5), (3520.0, 0.15)], release=0.9))

    print(f"\n{len(list(OUT.glob('*.wav')))} sounds written to {OUT}")
    return 0


if __name__ == "__main__":
    sys.exit(main())