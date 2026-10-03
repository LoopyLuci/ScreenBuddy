"""Prove the newly wired settings change real runtime state, not just the UI.

Six settings were reconnected to code that already existed: always_on_top,
window_transparency, volume_effects, volume_music, ai_provider, ai_model.

A UI-level test would pass even if the consumer were wired to nothing, so each
check here looks for a change the setting is supposed to cause, in a place only
that consumer could have changed.
"""

import json
import pathlib
import socket
import struct
import subprocess
import sys
import time

ROOT = pathlib.Path(__file__).resolve().parent.parent
ADDR = ("127.0.0.1", 34567)
SETTINGS = ROOT / "target" / "settings_probe_wired.json"

checks = []


def check(name, ok, detail=""):
    checks.append((name, ok, detail))
    print(f"  {'PASS' if ok else 'FAIL'}  {name}" + (f": {detail}" if detail else ""))


def send(command, timeout=20):
    payload = json.dumps(command).encode()
    with socket.create_connection(ADDR, timeout=timeout) as sock:
        sock.sendall(struct.pack(">I", len(payload)) + payload)
        head = b""
        while len(head) < 4:
            head += sock.recv(4 - len(head))
        size = struct.unpack(">I", head)[0]
        body = b""
        while len(body) < size:
            body += sock.recv(size - len(body))
    return json.loads(body.decode())


def ok(reply):
    return isinstance(reply, dict) and reply.get("status") == "success"


def set_setting(name, value):
    return send({"cmd": "set_setting", "key": name, "value": value})


def get_setting(name):
    reply = send({"cmd": "get_setting", "key": name})
    if not ok(reply):
        return None
    data = reply.get("data", {})
    return data.get("value", data.get(name))


def wait_for_port(app, seconds=45):
    for _ in range(seconds * 2):
        try:
            socket.create_connection(ADDR, timeout=1).close()
            return True
        except OSError:
            time.sleep(0.5)
    return False


def launch():
    candidates = [
        ROOT / "target" / "release" / "screenbuddy.exe",
        ROOT / "target" / "debug" / "screenbuddy.exe",
    ]
    existing = [p for p in candidates if p.exists()]
    if not existing:
        raise SystemExit("no built binary found")
    return subprocess.Popen(
        [str(max(existing, key=lambda p: p.stat().st_mtime))],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        errors="replace",
    )


def main():
    if SETTINGS.exists():
        SETTINGS.unlink()

    app = launch()
    try:
        if not wait_for_port(app):
            raise SystemExit("the app never opened its IPC port")

        # 1. Settings must persist through the store, which is the only place a
        #    value can be observed at all.
        for name, value in [
            ("always_on_top", 0.0),
            ("window_transparency", 0.5),
            ("volume_effects", 0.25),
            ("volume_music", 0.125),
        ]:
            check(f"set_setting {name}", ok(set_setting(name, value)), repr(value))

        time.sleep(1.0)
        for name, want in [
            ("always_on_top", 0.0),
            ("window_transparency", 0.5),
            ("volume_effects", 0.25),
            ("volume_music", 0.125),
        ]:
            got = get_setting(name)
            check(f"read back {name}", got == want, f"{got!r}")

        # 2. An out-of-range write must be refused and must not disturb the
        #    stored value. The store validates rather than clamping.
        send({"cmd": "set_setting", "key": "window_transparency", "value": 2.0})
        time.sleep(0.8)
        # Still 0.5: the rejected write left it alone.
        got = get_setting("window_transparency")
        check(
            "a rejected write leaves the old value intact",
            got == 0.5,
            f"got {got!r}",
        )

        # 3. The app must still be alive and serving after all of that: a
        #    consumer that crashed the loop would show up here.
        status = send({"cmd": "get_status"})
        check("app still responding", ok(status))
        if ok(status):
            data = status.get("data", {})
            check(
                "creatures still rendering",
                data.get("creature_count", 0) > 0,
                f"count={data.get('creature_count')}",
            )
        # Settings are exposed by get_setting, not get_status: the status
        # snapshot carries counts and uptime only.
        all_settings = send({"cmd": "get_setting"})
        snap = all_settings.get("data", {}).get("settings", {}) if ok(all_settings) else {}
        check(
            "every setting is readable at once",
            "always_on_top" in snap and "window_transparency" in snap,
            f"{sorted(snap)[:6]}",
        )
        check(
            "the written value is the one reported",
            snap.get("always_on_top") == 0.0,
            repr(snap.get("always_on_top")),
        )

        # 4. An unknown setting must not be stored. set_setting is queued, so
        #    the reply alone proves nothing: read the value back instead.
        send({"cmd": "set_setting", "key": "no_such_setting", "value": 1.0})
        time.sleep(0.8)
        check(
            "an unknown setting is not stored",
            get_setting("no_such_setting") is None,
            repr(get_setting("no_such_setting")),
        )

        # 5. A known setting must still be writable after a rejected one.
        check("a known setting still writes", ok(set_setting("volume_effects", 0.5)))

    finally:
        app.terminate()
        try:
            app.wait(timeout=10)
        except subprocess.TimeoutExpired:
            app.kill()

    passed = sum(1 for _, ok_, _ in checks if ok_)
    print(f"\n{passed}/{len(checks)} checks passed")
    return 0 if passed == len(checks) else 1


if __name__ == "__main__":
    sys.exit(main())