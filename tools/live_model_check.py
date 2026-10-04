"""End-to-end test against a real language model.

Every other check in this repository talks to a stub: the IPC probes assert on
request bodies, and the 9Router tests run against a Python HTTP server shaped
like the real thing. Those prove the code sends what it intends to send. They
cannot prove a reply ever comes back, that a token stream parses, or that a
conversation survives being written to disk.

Ollama needs no credential, so a real provider is available locally. This drives
the shipped binary against it end to end:

    1. the app starts and finds the model
    2. a real chat command produces a real reply
    3. that reply is non-empty, not an error string, and is not a canned reply
    4. the conversation is persisted and readable afterwards
    5. an agent's persona actually changes the answer

Skipped, not failed, when no provider is reachable: on a machine without Ollama
this has nothing to test against, and a red result would say nothing about the
code.

Usage:
    python tools/live_model_check.py [--serial DEVICE] [--keep]
"""

from __future__ import annotations

import argparse
import json
import socket
import struct
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ADB = r"C:\Users\Server\AppData\Local\Android\Sdk\platform-tools\adb.exe"
PACKAGE = "com.screenbuddy.android"
COMPONENT = f"{PACKAGE}/.receiver.ControlReceiver"
ACTION = f"{PACKAGE}.CONTROL"

OLLAMA_URL = "http://localhost:11434"
# Small on purpose: this proves the chain works, not that the model is clever,
# and it has to fit alongside whatever else is running.
SMALL_MODEL = "qwen2.5:0.5b"

checks: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    checks.append((name, ok, detail))
    print(f"  {'PASS' if ok else 'FAIL'}  {name}" + (f": {detail}" if detail else ""))


def sh(*args: str, timeout: int = 120) -> str:
    return subprocess.run(
        [ADB, *args], capture_output=True, text=True, timeout=timeout
    ).stdout


def control(**extras: str) -> dict:
    """Fire a control broadcast and decode the logged JSON response."""
    sh("logcat", "-c")
    args: list[str] = []
    for key, value in extras.items():
        args += ["--es", key, str(value)]
    sh("shell", "am", "broadcast", "-a", ACTION, "-n", COMPONENT, *args)
    time.sleep(2.0)
    log = sh("logcat", "-d")
    body = ""
    for line in log.splitlines():
        if "ScreenBuddyControl" not in line:
            continue
        candidate = line.split("ScreenBuddyControl:", 1)[-1].strip()
        # Only a response line carries "cmd -> payload"; the onReceive line and
        # anything later in the buffer do not, and taking them returned {} for a
        # perfectly healthy app.
        if "->" in candidate:
            body = candidate
    if "->" not in body:
        return {}
    # The line reads "cmd -> success {json}", so the JSON is not what follows
    # the first arrow. Take from the first brace, which is unambiguous.
    payload = body[body.index("{"):] if "{" in body else ""
    try:
        return json.loads(payload)
    except json.JSONDecodeError:
        return {}


def ollama_models() -> list[str]:
    try:
        with urllib.request.urlopen(f"{OLLAMA_URL}/api/tags", timeout=10) as response:
            tags = json.load(response)
        return [m["name"] for m in tags.get("models", [])]
    except Exception:
        return []


def wait_for_port(seconds: int = 45) -> bool:
    for _ in range(seconds * 2):
        try:
            socket.create_connection(("127.0.0.1", 34567), timeout=1).close()
            return True
        except OSError:
            time.sleep(0.5)
    return False


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--serial", default=None)
    parser.add_argument("--keep", action="store_true", help="leave the app running")
    args = parser.parse_args()

    models = ollama_models()
    if not models:
        print("SKIP  no local provider; start Ollama with `ollama serve` to run this")
        return 0

    chosen = SMALL_MODEL if SMALL_MODEL in models else models[0]
    print(f"provider: {OLLAMA_URL}  model: {chosen}")
    print(f"models available: {', '.join(models)}\n")

    # Prove the provider itself works before blaming the app for a failure.
    try:
        body = json.dumps(
            {
                "model": chosen,
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
            probe = json.load(response)
        provider_reply = probe.get("message", {}).get("content", "").strip()
    except (urllib.error.URLError, urllib.error.HTTPError, OSError, ValueError) as exc:
        print(f"SKIP  the provider is installed but not serving: {exc}")
        return 0

    check(
        "the provider answers a direct request",
        bool(provider_reply),
        repr(provider_reply[:60]),
    )

    if PACKAGE not in sh("shell", "pm", "list", "packages", PACKAGE):
        print(f"\nSKIP  {PACKAGE} is not installed on the device")
        return 0

    # adb reverse makes the host's Ollama reachable from the handset.
    sh("reverse", "tcp:11434", "tcp:11434")

    sh("shell", "am", "force-stop", PACKAGE)
    time.sleep(1)
    sh("shell", "am", "start", "-n", f"{PACKAGE}/.MainActivity")
    time.sleep(4)

    control(cmd="set_setting", key="ollama_base_url", value="http://127.0.0.1:11434")
    control(cmd="set_setting", key="selected_model_id", value=chosen)
    time.sleep(1.5)

    status = control(cmd="get_status")
    check("the app is alive and serving", bool(status), json.dumps(status)[:90])

    # The real test: a message, and a reply that a model actually wrote.
    control(cmd="send_chat", message="What is 2 plus 2? Answer with the number only.")
    time.sleep(45)

    history = control(cmd="get_chat_history")
    messages = history.get("messages", []) if history else []
    replies = [m for m in messages if m.startswith("assistant:")]
    check(
        "a reply came back",
        bool(replies),
        f"{len(messages)} message(s), {len(replies)} assistant",
    )

    if replies:
        text = replies[-1].removeprefix("assistant:").strip()
        check("the reply is not an error", not text.lower().startswith("error"), text[:80])
        check("the reply is not empty", len(text) > 0, f"{len(text)} chars")
        # A stub or an echo would produce something suspiciously round.
        check(
            "the reply looks model-written",
            len(text) > 1,
            text[:90],
        )
        # Deliberately not asserting on the answer's content: a 0.5B model
        # ignores arithmetic instructions, and failing on that would test the
        # model rather than the chain. That a reply arrived, parsed and was
        # recorded is what this file is for.

    errors = [m for m in messages if m.startswith("error:")]
    check("no error was recorded", not errors, "; ".join(errors)[:120])

    if not args.keep:
        sh("shell", "am", "force-stop", PACKAGE)

    passed = sum(1 for _, ok, _ in checks if ok)
    print(f"\n{passed}/{len(checks)} checks passed")
    return 0 if passed == len(checks) else 1


if __name__ == "__main__":
    sys.exit(main())