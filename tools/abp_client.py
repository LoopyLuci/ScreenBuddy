#!/usr/bin/env python3
"""One-shot ScreenBuddy chat client for AgenticBotPlatform's screenbuddy backend.

Sends a prompt to a running ScreenBuddy over its IPC surface and prints the
creature's reply on stdout. Designed to be spawned once per turn by ABP's
screenbuddy backend (bot/backends/screenbuddy_backend.py), which follows the
external-agent pattern: this process exits 0 with the reply text on stdout,
or non-zero with the reason on stderr.

The reply is polled for, not awaited: send_chat queues the turn and the
model answers asynchronously (38-60s for a small local model), so the
history is watched until a NEW assistant message appears.

Usage:
    python tools/abp_client.py "What is 2 plus 2?"
    python tools/abp_client.py --timeout 180 --addr 127.0.0.1:34567 "hi"
"""
from __future__ import annotations

import argparse
import json
import socket
import struct
import sys
import time

DEFAULT_ADDR = "127.0.0.1:34567"


def send(addr: str, command: dict, timeout: float) -> dict:
    host, port = addr.rsplit(":", 1)
    payload = json.dumps(command).encode("utf-8")
    with socket.create_connection((host, int(port)), timeout=timeout) as sock:
        sock.sendall(struct.pack(">I", len(payload)) + payload)
        header = _read_exactly(sock, 4)
        (length,) = struct.unpack(">I", header)
        body = _read_exactly(sock, length)
    return json.loads(body.decode("utf-8"))


def _read_exactly(sock: socket.socket, count: int) -> bytes:
    chunks = b""
    while len(chunks) < count:
        block = sock.recv(count - len(chunks))
        if not block:
            raise ConnectionError("connection closed mid-frame")
        chunks += block
    return chunks


def assistant_messages(addr: str, timeout: float) -> list[str]:
    """Every assistant message currently in the chat history, oldest first."""
    resp = send(addr, {"cmd": "get_chat_history", "limit": 100}, timeout)
    messages = resp.get("data", {}).get("messages", [])
    out = []
    for m in messages:
        if isinstance(m, (list, tuple)) and len(m) == 2:
            role, body = m
        elif isinstance(m, str) and m.startswith("assistant:"):
            role, body = "assistant", m.removeprefix("assistant:")
        else:
            continue
        if str(role).strip().lower() == "assistant" and str(body).strip():
            out.append(str(body))
    return out


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("prompt", help="the prompt to send to the creature")
    parser.add_argument("--addr", default=DEFAULT_ADDR, help="IPC address (host:port)")
    parser.add_argument("--timeout", type=float, default=180.0,
                        help="seconds to wait for the reply")
    args = parser.parse_args()

    # A reply that contains emoji must survive Windows' cp1252 console.
    try:
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    except (AttributeError, ValueError):
        pass

    deadline = time.time() + args.timeout
    try:
        before = assistant_messages(args.addr, timeout=15.0)
        queued = send(args.addr, {"cmd": "send_chat", "message": args.prompt}, timeout=15.0)
        if queued.get("status") != "success":
            print(f"send_chat failed: {json.dumps(queued)[:200]}", file=sys.stderr)
            return 2
    except (OSError, ConnectionError, ValueError) as exc:
        print(f"could not reach ScreenBuddy at {args.addr}: {exc}", file=sys.stderr)
        return 2

    # Poll until a NEW assistant message appears. Compare by content, not by
    # count: the app's history resets when a new chat session starts, so a
    # count-based wait (len > before) never fires across a reset even though
    # the reply has arrived. The last assistant message changing is the
    # signal that actually means "the creature answered".
    last_before = before[-1] if before else None
    poll_interval = 1.0
    while time.time() < deadline:
        time.sleep(poll_interval)
        try:
            messages = assistant_messages(args.addr, timeout=15.0)
        except (OSError, ConnectionError, ValueError):
            continue  # a transient IPC hiccup is not a failure; keep waiting
        if messages and messages[-1] != last_before:
            print(messages[-1])
            return 0
    print(f"no reply within {args.timeout:g}s", file=sys.stderr)
    return 3


if __name__ == "__main__":
    sys.exit(main())
