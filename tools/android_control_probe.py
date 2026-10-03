#!/usr/bin/env python3
"""Drive the ScreenBuddy Android app over its control broadcast surface.

This is the Android counterpart to tools/ipc_probe.py and mcp/test_mcp.py: it
checks the same command vocabulary the desktop exposes, but through
`am broadcast` rather than TCP.

Requires a connected device and the app installed.

Usage:
    python tools/android_control_probe.py [--serial DEVICE]
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
import time

PACKAGE = "com.screenbuddy.android"
COMPONENT = f"{PACKAGE}/.receiver.ControlReceiver"
ACTION = f"{PACKAGE}.CONTROL"
TAG = "ScreenBuddyControl"


def adb(serial: str | None, *args: str, timeout: int = 60) -> subprocess.CompletedProcess:
    cmd = ["adb"]
    if serial:
        cmd += ["-s", serial]
    cmd += list(args)
    return subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)


def send(serial: str | None, extras: list[str]) -> tuple[int, str]:
    """Fire one control broadcast and return (result_code, logged_response)."""
    adb(serial, "logcat", "-c")
    args = ["shell", "am", "broadcast", "-a", ACTION, "-n", COMPONENT] + extras
    proc = adb(serial, *args)
    m = re.search(r"result=(\d+)", proc.stdout)
    code = int(m.group(1)) if m else -1
    time.sleep(0.6)
    log = adb(serial, "logcat", "-d").stdout
    body = ""
    for line in log.splitlines():
        if TAG in line and "onReceive" not in line:
            body = line.split(f"{TAG}:", 1)[-1].strip()
    return code, body


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--serial", default=None)
    args = parser.parse_args()
    serial = args.serial

    installed = adb(serial, "shell", "pm", "list", "packages", PACKAGE)
    if PACKAGE not in installed.stdout:
        print(f"{PACKAGE} is not installed. Build and install it first.")
        return 2

    receivers = adb(serial, "shell", "pm", "query-receivers", "-a", ACTION).stdout
    if "ControlReceiver" not in receivers:
        print("ControlReceiver is not registered on the device.")
        return 2

    checks: list[tuple[str, bool, str]] = []

    def check(name: str, extras: list[str], want_code: int, want_in: str | None = None) -> None:
        code, body = send(serial, extras)
        ok = code == want_code and (want_in is None or want_in in body)
        detail = "" if ok else f"got code={code} body={body[:110]}"
        checks.append((name, ok, detail))

    check("ping", ["--es", "cmd", "ping"], 0, "pong")
    check("get_status", ["--es", "cmd", "get_status"], 0, "animation_state")
    check(
        "set valid animation",
        ["--es", "cmd", "set_animation", "--es", "state", "celebrate"],
        0,
        "queued",
    )
    check(
        "reject invalid animation",
        ["--es", "cmd", "set_animation", "--es", "state", "bogus"],
        1,
        "must be one of",
    )
    check(
        "set visibility",
        ["--es", "cmd", "set_creature_visible", "--ez", "visible", "false"],
        0,
        "queued",
    )
    check(
        "set and read setting",
        ["--es", "cmd", "set_setting", "--es", "key", "master_volume", "--es", "value", "0.42"],
        0,
        "queued",
    )
    check(
        "read setting back",
        ["--es", "cmd", "get_setting", "--es", "key", "master_volume"],
        0,
        "0.42",
    )
    check("unknown command rejected", ["--es", "cmd", "nonsense"], 1, "unsupported command")
    check(
        "memory ingest then search",
        [
            "--es", "cmd", "memory_ingest",
            "--es", "source", "memory://probe",
            "--es", "content", "The domestic feline grooms itself often.",
        ],
        0,
        "chunks",
    )
    check(
        "memory search finds it",
        ["--es", "cmd", "memory_search", "--es", "query", "feline grooming"],
        0,
        "feline",
    )

    # Confirm the mutations actually landed rather than merely being accepted.
    _, status = send(serial, ["--es", "cmd", "get_status"])
    checks.append(
        (
            "state reflects earlier commands",
            '"animation_state":"celebrate"' in status and '"visible":false' in status,
            "" if "celebrate" in status else f"status={status[:110]}",
        )
    )

    width = max(len(n) for n, _, _ in checks)
    for name, ok, detail in checks:
        print(f"  {name:<{width}}  {'PASS' if ok else 'FAIL'}{'  ' + detail if detail else ''}")
    failed = [n for n, ok, _ in checks if not ok]
    print(f"\n{len(checks) - len(failed)}/{len(checks)} Android control checks passed")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())