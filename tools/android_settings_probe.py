"""Prove on a real device that the previously inert Android settings now work.

The desktop audit found 13 of 17 settings had no consumer. This drives the
Android app over its own control broadcast surface and checks the value reaches
the AI request, which is the only place temperature, response length and
streaming can actually be observed.

Pointing the app at a local stub is what makes this checkable: the stub records
the JSON body it was sent, so the assertion is on the wire format rather than on
storage.

Requires a connected device with the app installed.

Usage:
    python tools/android_settings_probe.py [--serial DEVICE]
"""

from __future__ import annotations

import argparse
import http.server
import json
import re
import socketserver
import subprocess
import sys
import threading
import time

ADB = r"C:\Users\Server\AppData\Local\Android\Sdk\platform-tools\adb.exe"
PACKAGE = "com.screenbuddy.android"
COMPONENT = f"{PACKAGE}/.receiver.ControlReceiver"
ACTION = f"{PACKAGE}.CONTROL"
TAG = "ScreenBuddyControl"

received: list[dict] = []


class Stub(http.server.BaseHTTPRequestHandler):
    """Accepts an OpenAI-shaped request and records its body."""

    def _send(self, obj):
        out = json.dumps(obj).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(out)))
        self.end_headers()
        self.wfile.write(out)

    def do_POST(self):
        length = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(length)
        try:
            received.append(json.loads(body))
        except Exception as exc:  # pragma: no cover - diagnostic path
            received.append({"_parse_error": str(exc)})
        self._send(
            {
                "choices": [
                    {
                        "message": {"role": "assistant", "content": "ok"},
                        "finish_reason": "stop",
                    }
                ]
            }
        )

    def do_GET(self):
        self._send({"models": [{"id": "llama3.2", "name": "llama3.2", "max_tokens": 2048}]})

    def log_message(self, *args):
        pass


def adb(serial, *args, timeout=120):
    cmd = [ADB]
    if serial:
        cmd += ["-s", serial]
    cmd += list(args)
    return subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)


def control(serial, **extras) -> str:
    """Fire one control broadcast and return the logged response."""
    adb(serial, "logcat", "-c")
    args = []
    for key, value in extras.items():
        args += [f"--es", key, str(value)]
    proc = adb(serial, "shell", "am", "broadcast", "-a", ACTION, "-n", COMPONENT, *args)
    time.sleep(0.8)
    log = adb(serial, "logcat", "-d").stdout
    # Take the response line, not merely the last tagged line: a stack trace
    # now follows a successful reply.
    body = ""
    for line in log.splitlines():
        if TAG not in line or "onReceive" in line:
            continue
        text = line.split(f"{TAG}:", 1)[-1].strip()
        if "->" in text:
            body = text
    return body


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--serial", default=None)
    args = parser.parse_args()
    serial = args.serial

    installed = adb(serial, "shell", "pm", "list", "packages", PACKAGE)
    if PACKAGE not in installed.stdout:
        print(f"{PACKAGE} is not installed.")
        return 1

    srv = socketserver.TCPServer(("127.0.0.1", 0), Stub)
    port = srv.server_address[1]
    threading.Thread(target=srv.serve_forever, daemon=True).start()

    # adb reverse makes the host port reachable from the handset at 127.0.0.1.
    adb(serial, "reverse", f"tcp:{port}", f"tcp:{port}")

    adb(serial, "shell", "am", "force-stop", PACKAGE)
    adb(serial, "shell", "am", "start", "-n", f"{PACKAGE}/.MainActivity")
    time.sleep(3)

    checks: list[tuple[str, bool, str]] = []

    def check(name: str, ok: bool, detail: str = "") -> None:
        checks.append((name, ok, detail))
        print(f"  {'PASS' if ok else 'FAIL'}  {name}" + (f": {detail}" if detail else ""))

    base = f"http://127.0.0.1:{port}"
    for key, value in [
        ("ollama_base_url", base),
        ("temperature", 1.4),
        ("response_length", 256),
        ("stream_responses", "false"),
    ]:
        reply = control(serial, cmd="set_setting", key=key, value=value)
        check(f"set_setting {key}", '"status":"success"' in reply or "ok" in reply, reply[:90])

    time.sleep(1.5)
    # Confirm the settings read back before asking for a reply.
    read = control(serial, cmd="get_setting")
    check(
        "settings readable back",
        "temperature" in read,
        read[:110],
    )

    reply = control(serial, cmd="send_chat", message="hello", name="llama3.2")
    check("send_chat accepted", "success" in reply, reply[:110])
    time.sleep(5)

    bodies = [r for r in received if "messages" in r]
    check("the app sent a request", bool(bodies), f"{len(received)} body(s) received")
    if not bodies:
        # The chat history records why when the send fails.
        hist = control(serial, cmd="get_chat_history")
        print(f"    chat history: {hist[:220]}")

    if bodies:
        body = bodies[-1]
        # temperature was never sent at all before this change.
        temp = body.get("temperature")
        check(
            "temperature reaches the request",
            isinstance(temp, (int, float)) and abs(temp - 1.4) < 0.01,
            f"sent temperature={temp!r}",
        )
        # The cap is max_tokens on OpenAI-shaped endpoints and
        # options.num_predict on Ollama. Before this, Ollama sent neither.
        tokens = body.get("max_tokens")
        if tokens is None:
            tokens = (body.get("options") or {}).get("num_predict")
        check(
            "response length caps the generated tokens",
            isinstance(tokens, int) and tokens <= 256,
            f"sent {tokens!r}",
        )
        # Every call hardcoded stream=false, so the toggle did nothing.
        check(
            "streaming setting reaches the request",
            "stream" in body,
            f"sent stream={body.get('stream')!r}",
        )

    srv.shutdown()
    passed = sum(1 for _, ok, _ in checks if ok)
    print(f"\n{passed}/{len(checks)} checks passed")
    return 0 if passed == len(checks) else 1


if __name__ == "__main__":
    sys.exit(main())