"""The ScreenBuddy IPC surface, for the pieces that run inside ABP.

Same framed-JSON protocol tools/abp_client.py speaks, as a library: a
4-byte big-endian length, the JSON command, a length-prefixed JSON reply.
Every call raises ConnectionError/OSError when the creature is not
running — callers treat that as "not set up" and move on, the same
pre-flight the backend's ready() performs.
"""
from __future__ import annotations

import json
import socket
import struct
from typing import Any, Optional

DEFAULT_ADDR = "127.0.0.1:34567"


def addr_from_settings(settings: Optional[dict]) -> str:
    if settings:
        return str(settings.get("addr") or DEFAULT_ADDR)
    return DEFAULT_ADDR


def send(addr: str, command: dict, timeout: float = 10.0) -> dict:
    """One request/response round trip over the creature's IPC port."""
    host, _, port = addr.rpartition(":")
    payload = json.dumps(command).encode("utf-8")
    with socket.create_connection((host or "127.0.0.1", int(port)), timeout=timeout) as sock:
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


def reachable(addr: str, timeout: float = 2.0) -> bool:
    """Is a ScreenBuddy actually listening? The memory source and the event
    watcher both no-op when it isn't, instead of failing a sync."""
    host, _, port = addr.rpartition(":")
    try:
        with socket.create_connection((host or "127.0.0.1", int(port)), timeout=timeout):
            return True
    except OSError:
        return False
