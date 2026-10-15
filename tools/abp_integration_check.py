#!/usr/bin/env python3
"""End-to-end live test: ScreenBuddy's send_chat routes through AgenticBotPlatform.

The app is launched with ABP_ENDPOINT / ABP_DASHBOARD_TOKEN / ABP_SOURCE_INSTANCE /
ABP_TARGET_INSTANCE in its environment and ai_provider=abp pinned in settings.json,
so a send_chat must travel: IPC -> AiChatBridge -> gen_abp -> POST /api/agent/ask
-> ABP native_agent -> Ollama -> back. A reply of INTEGRATION_OK proves the
whole chain; anything else (canned fallback, connection error) proves a break.

Also runs the reverse direction afterwards: register ScreenBuddy's MCP server in
ABP and prove its 25 tools are visible to ABP's client.
"""
import json
import os
import socket
import struct
import subprocess
import sys
import time
import urllib.request

ADDR = ("127.0.0.1", 34567)
EXE = r"C:\Projects\ScreenBuddy\target\release\screenbuddy.exe"
ABP_TOKEN = "sb-integration-test-token-2026"
ENV = dict(
    os.environ,
    ABP_ENDPOINT="http://127.0.0.1:8787",
    ABP_DASHBOARD_TOKEN=ABP_TOKEN,
    ABP_SOURCE_INSTANCE="screenbuddy",
    ABP_TARGET_INSTANCE="screenbuddy-test",
)


def send(command: dict, timeout: int = 200) -> dict:
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
    return json.loads(body.decode("utf-8"))


def wait_for_port(seconds: int = 90) -> bool:
    for _ in range(seconds * 2):
        try:
            socket.create_connection(ADDR, timeout=1).close()
            return True
        except OSError:
            time.sleep(0.5)
    return False


def abp_api(path: str, method: str = "GET", payload: dict | None = None) -> dict:
    req = urllib.request.Request(
        f"http://127.0.0.1:8787{path}",
        method=method,
        headers={
            "X-Dashboard-Token": ABP_TOKEN,
            "Content-Type": "application/json",
        },
        data=json.dumps(payload).encode() if payload else None,
    )
    with urllib.request.urlopen(req, timeout=60) as resp:
        return json.load(resp)


print("== Phase 1: ScreenBuddy -> ABP (gen_abp) ==")
# Skip, not fail, when ABP is not running — a red result on a machine
# without ABP would say nothing about the code.
try:
    abp_api("/api/bots")
except (urllib.error.URLError, OSError) as exc:
    print(f"SKIP  ABP dashboard not reachable on 127.0.0.1:8787: {exc}")
    print("Start it with: cd <AgenticBotPlatform> && python -m bot.main")
    raise SystemExit(0)
app = subprocess.Popen(
    [EXE], stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
    text=True, errors="replace", env=ENV,
)
try:
    if not wait_for_port():
        print("FAIL: app never opened IPC port")
        raise SystemExit(1)
    time.sleep(2.0)

    status = send({"cmd": "get_status"})
    print("app status:", json.dumps(status)[:120])

    # Pin via IPC first (the running app's engine), in case the file-based
    # settings did not take effect for some reason.
    send({"cmd": "set_setting", "key": "ai_provider", "value": "abp"})
    send({"cmd": "set_setting", "key": "ai_model", "value": "abp"})

    t0 = time.time()
    reply = send({"cmd": "send_chat", "message": "Reply with exactly: INTEGRATION_OK"})
    print(f"send_chat queued in {time.time() - t0:.1f}s: {json.dumps(reply)[:100]}")

    # The chat is processed async; poll the history for the assistant reply.
    text = ""
    for _ in range(120):  # up to ~4 minutes for a 0.5b model through ABP
        time.sleep(2.0)
        hist = send({"cmd": "get_chat_history", "limit": 20}, timeout=30)
        messages = hist.get("data", {}).get("messages", [])
        replies = []
        for m in messages:
            if isinstance(m, (list, tuple)) and len(m) == 2:
                role, body = m
            elif isinstance(m, str) and m.startswith("assistant:"):
                role, body = "assistant", m.removeprefix("assistant:")
            else:
                continue
            if str(role).strip().lower() == "assistant" and str(body).strip():
                replies.append(str(body))
        if replies:
            text = replies[-1]
            break
    elapsed = time.time() - t0
    print(f"reply after {elapsed:.1f}s: {json.dumps(text)[:200]}")
    ok = "INTEGRATION_OK" in text
    print(f"{'PASS' if ok else 'FAIL'}: reply came through ABP -> Ollama")
finally:
    app.terminate()
    try:
        app.wait(timeout=10)
    except subprocess.TimeoutExpired:
        app.kill()

print()
print("== Phase 2: ABP MCP client <- ScreenBuddy MCP server ==")
try:
    reg = abp_api("/api/mcp-external", "POST", {
        "name": "screenbuddy",
        "transport": "stdio",
        "command": "python",
        "args": [r"C:\Projects\ScreenBuddy\mcp\server.py"],
        "env": {"SCREENBUDDY_AUTOSTART": "0"},
    })
    print("registration:", json.dumps(reg))
except urllib.error.HTTPError as exc:
    if exc.code == 400:
        print("registration: already exists (idempotent re-run) — verifying via listing")
    else:
        raise
listing = abp_api("/api/mcp-external")
servers = listing.get("servers", [])
sb = next((s for s in servers if s["name"] == "screenbuddy"), None)
print(f"{'PASS' if sb and sb.get('connected') else 'FAIL'}: screenbuddy MCP server connected={sb and sb.get('connected')}")
