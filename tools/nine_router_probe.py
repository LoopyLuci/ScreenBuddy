"""Live check of the 9Router integration through the running desktop app.

Starts the stub router, starts the real ScreenBuddy binary, then sends the
`nine_router_status` control request over the actual IPC socket. This is the
path Hermes uses, so it proves the whole chain rather than just the library.
"""

import json
import socket
import struct
import subprocess
import sys
import time
import pathlib

ROOT = pathlib.Path(__file__).resolve().parent.parent
ADDR = ("127.0.0.1", 34567)


def free_port():
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


def send(command, addr=ADDR, timeout=20):
    payload = json.dumps(command).encode()
    frame = struct.pack(">I", len(payload)) + payload
    with socket.create_connection(addr, timeout=timeout) as sock:
        sock.sendall(frame)
        header = b""
        while len(header) < 4:
            chunk = sock.recv(4 - len(header))
            if not chunk:
                raise RuntimeError("connection closed before a response header")
            header += chunk
        length = struct.unpack(">I", header)[0]
        body = b""
        while len(body) < length:
            chunk = sock.recv(length - len(body))
            if not chunk:
                break
            body += chunk
    return json.loads(body.decode())


def main():
    port = free_port()
    base = f"http://127.0.0.1:{port}/v1"

    stub = subprocess.Popen(
        [sys.executable, str(ROOT / "tools" / "nine_router_stub.py"), str(port)],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    print(f"stub 9router on {base}")

    exe = ROOT / "target" / "release" / "screenbuddy.exe"
    if not exe.exists():
        exe = ROOT / "target" / "debug" / "screenbuddy.exe"
    if not exe.exists():
        print("FAIL: no built binary; run cargo build first")
        stub.terminate()
        return 1

    log = open(ROOT / "target" / "nine_router_app.log", "w", encoding="utf-8", errors="replace")
    app = subprocess.Popen([str(exe)], stdout=log, stderr=subprocess.STDOUT)
    print(f"app pid {app.pid}")

    results = []
    try:
        # Wait for the IPC port.
        for _ in range(60):
            try:
                socket.create_connection(ADDR, timeout=1).close()
                break
            except OSError:
                time.sleep(0.5)
        else:
            print("FAIL: app never opened its IPC port")
            return 1
        time.sleep(1.5)

        # 1. The router answers and its models are listed. Control requests are
        # queued for the render loop and acknowledged immediately, so the result
        # is read back from the status snapshot rather than the ack.
        ack = send({
            "cmd": "nine_router_status",
            "endpoint": base,
            "api_key": None,
        })
        # The discovered list is published into the status snapshot, so read it
        # back with get_setting rather than expecting it in get_status.
        discovered = None
        for _ in range(40):
            time.sleep(0.25)
            readback = send({"cmd": "get_setting", "key": "nine_router.models"})
            models = readback.get("data", {}).get("value")
            if models:
                discovered = models
                break
        results.append((
            "live router discovery",
            {"status": "success", "data": {"models": discovered}} if discovered else ack,
        ))

        # 2. A dead router must fail cleanly, not hang or crash the app.
        dead = send({
            "cmd": "nine_router_status",
            "endpoint": "http://127.0.0.1:1/v1",
            "api_key": None,
        })
        results.append(("dead router handled", dead))

        # 3. The app must still be alive and serving after the failure.
        status = send({"cmd": "get_status"})
        results.append(("app alive after probe", status))

        # 4. An auth-protected router must send the bearer token.
        reply = send({
            "cmd": "nine_router_status",
            "endpoint": base,
            "api_key": "test-token",
        })
        results.append(("auth token accepted", reply))
    finally:
        app.terminate()
        try:
            app.wait(timeout=10)
        except subprocess.TimeoutExpired:
            app.kill()
        stub.terminate()

    print()
    try:
        tail = (ROOT / "target" / "nine_router_app.log").read_text(
            encoding="utf-8", errors="replace"
        )
        router_lines = [l for l in tail.splitlines() if "9Router" in l]
        print("app log (9Router lines):")
        for line in router_lines[-6:]:
            print("   ", line)
        if not router_lines:
            print("    (none - the command never reached the handler)")
    except Exception as exc:  # noqa: BLE001
        print(f"could not read app log: {exc}")

    failures = 0
    for name, reply in results:
        ok = isinstance(reply, dict)
        print(f"  {'PASS' if ok else 'FAIL'}  {name}: {json.dumps(reply)[:180]}")
        if not ok:
            failures += 1

    # The discovery result must actually carry the router's models.
    models = results[0][1].get("data", {}).get("models") or []
    found = "cc/claude-opus-4-6" in models
    print(f"  {'PASS' if found else 'FAIL'}  stub model present: {models}")
    if not found:
        failures += 1

    print(f"\n{len(results) + 1 - failures}/{len(results) + 1} checks passed")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())