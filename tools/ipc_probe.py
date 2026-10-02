"""End-to-end probe for the ScreenBuddy IPC control surface.

Runs against a live ScreenBuddy instance: sends length-prefixed JSON frames and
checks the replies. Used to verify the server is actually wired into the
running app rather than just compiling.

Usage:
    python tools/ipc_probe.py [--addr 127.0.0.1:34567]
"""

import argparse
import json
import socket
import struct
import sys
import time


def send(addr, command, timeout=5.0):
    """Send one command frame and return the decoded response."""
    host, port = addr.rsplit(":", 1)
    payload = json.dumps(command).encode("utf-8")
    with socket.create_connection((host, int(port)), timeout=timeout) as sock:
        sock.sendall(struct.pack(">I", len(payload)) + payload)
        header = _read_exactly(sock, 4)
        (length,) = struct.unpack(">I", header)
        body = _read_exactly(sock, length)
    return json.loads(body.decode("utf-8"))


def _read_exactly(sock, count):
    chunks = b""
    while len(chunks) < count:
        block = sock.recv(count - len(chunks))
        if not block:
            raise ConnectionError("connection closed mid-frame")
        chunks += block
    return chunks


def data_of(response):
    """Extract the payload from a success envelope, raising on error."""
    status = response.get("status")
    if status == "error":
        raise AssertionError(f"IPC error: {response.get('message')}")
    return response.get("data")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--addr", default="127.0.0.1:34567")
    parser.add_argument("--wait", type=float, default=10.0, help="seconds to wait for the app")
    args = parser.parse_args()

    # The app binds asynchronously after loading assets; retry until it answers.
    deadline = time.time() + args.wait
    last_error = None
    while time.time() < deadline:
        try:
            if send(args.addr, {"cmd": "ping"}) == {"status": "pong"}:
                break
        except (OSError, ConnectionError) as exc:
            last_error = exc
            time.sleep(0.5)
    else:
        print(f"FAIL: no ScreenBuddy instance answered on {args.addr}: {last_error}")
        return 1

    checks = []

    def check(name, fn):
        try:
            fn()
            checks.append((name, "PASS", ""))
        except Exception as exc:  # noqa: BLE001 - report every failure
            checks.append((name, "FAIL", str(exc)))

    def t_ping():
        assert send(args.addr, {"cmd": "ping"}) == {"status": "pong"}

    def t_status():
        payload = data_of(send(args.addr, {"cmd": "get_status"}))
        assert payload["creature_count"] > 0, "expected creatures to be loaded"
        assert payload["frames"] > 0, "expected frames to have rendered"
        assert payload["tool_count"] > 0, "expected agent tools to be registered"

    def t_creature_state():
        payload = data_of(send(args.addr, {"cmd": "list_creature_state"}))
        creatures = payload["creatures"]
        assert creatures, "expected at least one creature"
        first = next(iter(creatures.values()))
        assert "x" in first and "state" in first, f"missing fields in {first}"

    def t_set_animation():
        payload = data_of(
            send(args.addr, {"cmd": "set_animation", "id": "companion-bird-01", "state": "sleep"})
        )
        assert payload["queued"] is True, payload

    def t_move():
        payload = data_of(
            send(args.addr, {"cmd": "move_creature", "id": "companion-bird-01", "x": 300.0, "y": 200.0})
        )
        assert payload["queued"] is True, payload

    def t_visibility():
        payload = data_of(
            send(args.addr, {"cmd": "set_creature_visible", "id": "companion-bird-01", "visible": False})
        )
        assert payload["queued"] is True, payload
        # Restore, so a later probe still sees the creature.
        send(args.addr, {"cmd": "set_creature_visible", "id": "companion-bird-01", "visible": True})

    def t_setting_roundtrip():
        payload = data_of(send(args.addr, {"cmd": "set_setting", "key": "volume_master", "value": 0.42}))
        assert payload["queued"] is True, payload
        time.sleep(0.4)
        got = data_of(send(args.addr, {"cmd": "get_setting", "key": "volume_master"}))
        assert got["value"] == 0.42, f"expected 0.42, got {got}"

    def t_unknown_setting():
        response = send(args.addr, {"cmd": "get_setting", "key": "definitely-not-a-setting"})
        assert response.get("status") == "error", response

    def t_auto_cycle():
        payload = data_of(send(args.addr, {"cmd": "set_auto_cycle", "enabled": False}))
        assert payload["queued"] is True
        time.sleep(0.3)
        assert data_of(send(args.addr, {"cmd": "get_status"}))["auto_cycle"] is False
        send(args.addr, {"cmd": "set_auto_cycle", "enabled": True})

    def t_memory():
        payload = data_of(
            send(
                args.addr,
                {
                    "cmd": "memory_ingest",
                    "source": "probe://test",
                    "content": "the peregrine falcon dives at over two hundred miles per hour",
                },
            )
        )
        assert payload["queued"] is True
        time.sleep(0.4)
        assert data_of(send(args.addr, {"cmd": "get_status"}))["memory_chunks"] > 0

    def t_bad_frame_recovers():
        """A malformed frame must not wedge the connection."""
        host, port = args.addr.rsplit(":", 1)
        payload = b"{not json"
        with socket.create_connection((host, int(port)), timeout=5) as sock:
            sock.sendall(struct.pack(">I", len(payload)) + payload)
            header = _read_exactly(sock, 4)
            (length,) = struct.unpack(">I", header)
            body = json.loads(_read_exactly(sock, length).decode("utf-8"))
        assert body.get("status") == "error", body
        # Server must still answer afterwards.
        assert send(args.addr, {"cmd": "ping"}) == {"status": "pong"}

    for name, fn in [
        ("ping", t_ping),
        ("get_status reports live state", t_status),
        ("list_creature_state", t_creature_state),
        ("set_animation queues", t_set_animation),
        ("move_creature queues", t_move),
        ("set_creature_visible queues", t_visibility),
        ("setting write then read", t_setting_roundtrip),
        ("unknown setting errors", t_unknown_setting),
        ("auto_cycle toggles", t_auto_cycle),
        ("memory ingest", t_memory),
        ("malformed frame recovers", t_bad_frame_recovers),
    ]:
        check(name, fn)

    width = max(len(n) for n, _, _ in checks)
    for name, result, detail in checks:
        suffix = f"  {detail}" if detail else ""
        print(f"  {name:<{width}}  {result}{suffix}")

    failed = [n for n, r, _ in checks if r != "PASS"]
    print(f"\n{len(checks) - len(failed)}/{len(checks)} checks passed")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())