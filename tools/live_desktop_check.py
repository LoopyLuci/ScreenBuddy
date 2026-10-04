"""End-to-end test of the desktop binary against a real language model.

The Android app was validated against a real provider and immediately produced
two bugs that no stub could have found: models the user had pulled were rejected
as unknown, and every streamed reply failed to parse. The desktop binary had
390 Rust tests and had never completed a single real conversation.

This drives the shipped Windows executable over its own IPC surface, which is how
an agent would use it:

    1. the app starts and opens its control port
    2. a chat command produces a real reply from a real model
    3. the reply is non-empty, is not an error, and is not a canned string
    4. the conversation is persisted and readable afterwards

Skipped, not failed, when no provider is reachable: a red result on a machine
without Ollama would say nothing about the code.

Usage:
    python tools/live_desktop_check.py
"""

from __future__ import annotations

import json
import os
import socket
import struct
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ADDR = ("127.0.0.1", 34567)
OLLAMA_URL = "http://localhost:11434"
SMALL_MODEL = "qwen2.5:0.5b"
CONFIG_DIR = Path(os.environ["APPDATA"]) / "ScreenBuddy"

checks: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    checks.append((name, ok, detail))
    print(f"  {'PASS' if ok else 'FAIL'}  {name}" + (f": {detail}" if detail else ""))


def send(command: dict, timeout: int = 30) -> dict:
    payload = json.dumps(command).encode()
    with socket.create_connection(ADDR, timeout=timeout) as sock:
        sock.sendall(struct.pack(">I", len(payload)) + payload)
        header = b""
        while len(header) < 4:
            header += sock.recv(4 - len(header))
        size = struct.unpack(">I", header)[0]
        body = b""
        while len(body) < size:
            body += sock.recv(size - len(body))
    return json.loads(body.decode())


def ollama_models() -> list[str]:
    try:
        with urllib.request.urlopen(f"{OLLAMA_URL}/api/tags", timeout=10) as response:
            return [m["name"] for m in json.load(response).get("models", [])]
    except Exception:
        return []


def provider_works(model: str) -> tuple[bool, str]:
    """Prove the provider itself works, so a failure is attributable."""
    try:
        body = json.dumps(
            {
                "model": model,
                "messages": [{"role": "user", "content": "Reply with: PONG"}],
                "stream": False,
                "options": {"num_predict": 8},
            }
        ).encode()
        request = urllib.request.Request(
            f"{OLLAMA_URL}/api/chat", data=body,
            headers={"Content-Type": "application/json"},
        )
        with urllib.request.urlopen(request, timeout=300) as response:
            reply = json.load(response).get("message", {}).get("content", "").strip()
        return bool(reply), repr(reply[:50])
    except (urllib.error.URLError, urllib.error.HTTPError, OSError, ValueError) as exc:
        return False, str(exc)[:80]


def wait_for_port(seconds: int = 60) -> bool:
    for _ in range(seconds * 2):
        try:
            socket.create_connection(ADDR, timeout=1).close()
            return True
        except OSError:
            time.sleep(0.5)
    return False


def binary() -> Path | None:
    candidates = [
        ROOT / "target" / "release" / "screenbuddy.exe",
        ROOT / "target" / "debug" / "screenbuddy.exe",
    ]
    existing = [p for p in candidates if p.exists()]
    if not existing:
        return None
    return max(existing, key=lambda p: p.stat().st_mtime)


def read_transcript() -> list[dict]:
    """The persisted session, which is the durable proof a reply was stored."""
    path = CONFIG_DIR / "sessions.json"
    if not path.is_file():
        return []
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError):
        return []
    # A bare list of sessions; each message carries "text" and a capitalised
    # role, neither of which matched the shape I first assumed.
    sessions = data if isinstance(data, list) else data.get("sessions", [])
    messages: list[dict] = []
    for session in sessions:
        if isinstance(session, dict):
            messages.extend(session.get("messages", []))
    return messages


def main() -> int:
    models = ollama_models()
    if not models:
        print("SKIP  no local provider; start Ollama with `ollama serve` to run this")
        return 0

    chosen = SMALL_MODEL if SMALL_MODEL in models else models[0]
    print(f"provider: {OLLAMA_URL}  model: {chosen}\n")

    ok, detail = provider_works(chosen)
    if not ok:
        print(f"SKIP  the provider is installed but not serving: {detail}")
        return 0
    check("the provider answers a direct request", True, detail)

    exe = binary()
    if exe is None:
        print("SKIP  no built binary; run cargo build --release --bin screenbuddy")
        return 0

    # Start clean so the transcript check cannot pass on an earlier run.
    CONFIG_DIR.mkdir(parents=True, exist_ok=True)
    sessions = CONFIG_DIR / "sessions.json"
    backup = None
    if sessions.is_file():
        backup = CONFIG_DIR / "sessions.json.pre-live"
        sessions.replace(backup)

    app = subprocess.Popen(
        [str(exe)],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        errors="replace",
    )
    try:
        if not wait_for_port():
            raise SystemExit("the app never opened its control port")
        time.sleep(2.0)

        status = send({"cmd": "get_status"})
        check(
            "the app is alive and serving",
            isinstance(status, dict) and status.get("status") == "success",
            json.dumps(status)[:90],
        )

        check(
            "the model catalogue is visible",
            int(status.get("data", {}).get("model_count", 0)) > 0,
            f"model_count={status.get('data', {}).get('model_count')}",
        )

        # Ask a model to do something whose answer is checkable.
        send({"cmd": "send_chat", "message": f"Say hello in one short sentence."})
        time.sleep(60)

        messages = read_transcript()
        def body(message: dict) -> str:
            # Older entries used "content"; the store writes "text".
            return str(message.get("text") or message.get("content") or "").strip()

        def is_role(message: dict, role: str) -> bool:
            return str(message.get("role", "")).strip().lower() == role

        replies = [m for m in messages if is_role(m, "assistant") and body(m)]
        check(
            "a reply was persisted",
            bool(replies),
            f"{len(messages)} message(s), {len(replies)} assistant",
        )

        if replies:
            text = body(replies[-1])
            errors = [m for m in messages if is_role(m, "error")]
            check("no error was persisted", not errors, str(errors[:1])[:110])
            check("the reply is not empty", len(text) > 0, f"{len(text)} chars")
            # A canned or echoed string would be suspiciously short or identical
            # to what was asked.
            check(
                "the reply is not an echo of the prompt",
                "say hello in one short sentence" not in text.lower(),
                text[:80],
            )
            check("the reply is substantial", len(text) > 3, f"{len(text)} chars")
            # The important assertion: the text came from the model. A failure
            # report is also a non-empty, non-echo string, so shape checks alone
            # will happily pass on an app that never reached a model at all.
            lowered = text.lower()
            failure_markers = [
                "could not reach",
                "i'm sorry",
                "i am sorry",
                "as an ai",
                "error",
                "not available",
                "screenbuddy companion",
            ]
            found = [m for m in failure_markers if m in lowered]
            check(
                "the reply came from the model, not a fallback",
                not found,
                f"failure text in the reply: {found}" if found else text[:70],
            )
    finally:
        app.terminate()
        try:
            app.wait(timeout=15)
        except subprocess.TimeoutExpired:
            app.kill()
        # Leave the machine as it was found.
        if sessions.exists():
            sessions.unlink()
        if backup and backup.exists():
            backup.replace(sessions)

    passed = sum(1 for _, ok, _ in checks if ok)
    print(f"\n{passed}/{len(checks)} checks passed")
    return 0 if passed == len(checks) else 1


if __name__ == "__main__":
    sys.exit(main())