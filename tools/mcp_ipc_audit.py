"""Audit the MCP tools and IPC commands the way settings were audited.

Three defects in a row came from a list drifting away from what the code actually
does. The settings lists are now gated by dead_settings_scan.py; this does the
same job for the other two surfaces:

- MCP tools: defined in mcp/server.py, reachable only through the IPC commands
  they call. A tool whose command is not handled is a tool that errors at runtime.
- IPC commands: defined in GodotCommand, mapped to ControlRequest, and handled
  in the render loop. A command that is declared but never handled is accepted
  with "queued" and then silently dropped.
"""

import json
import pathlib
import re
import socket
import struct
import subprocess
import sys
import time

ROOT = pathlib.Path(__file__).resolve().parent.parent
MCP = ROOT / "mcp" / "server.py"
IPC = ROOT / "crates" / "screenbuddy-core" / "src" / "ipc.rs"
MAIN = ROOT / "crates" / "screenbuddy" / "src" / "main.rs"
ADDR = ("127.0.0.1", 34567)


def pascal(command):
    """`nine_router_status` -> `NineRouterStatus`.

    The wire format is snake_case via serde's rename_all, so comparing a JSON
    command name against the PascalCase enum requires converting.
    """
    return "".join(part.capitalize() for part in command.split("_"))


def mcp_tools():
    """Tool name -> the command string it sends."""
    text = MCP.read_text(encoding="utf-8")
    out = {}
    # Each tool body contains a call({"cmd": "...", ...}).
    for match in re.finditer(r"@mcp\.tool\(\)\s*\ndef (\w+)\((.*?)\n(?=@mcp\.tool|\Z)", text, re.S):
        name, body = match.group(1), match.group(2)
        cmd = re.search(r'["\']cmd["\']\s*:\s*["\']([a-z_]+)["\']', body)
        if cmd:
            out[name] = cmd.group(1)
    return out


def enum_block(text, name):
    """The body of a named enum, up to its own closing brace.

    Slicing to the *next* `pub enum` would swallow ControlRequest into
    GodotCommand's variants, which is exactly the bug this had.
    """
    start = text.index(f"pub enum {name}")
    # The enum ends at the first line that is just "}" at column 0.
    end = text.index("\n}\n", start) + 3
    return text[start:end]


def ipc_commands():
    """Commands declared on GodotCommand."""
    block = enum_block(IPC.read_text(encoding="utf-8"), "GodotCommand")
    return set(re.findall(r"^    ([A-Z][A-Za-z0-9]*)[\s({,]", block, re.M))


def mapped_commands():
    """Commands GodotCommand maps onto a ControlRequest."""
    text = IPC.read_text(encoding="utf-8")
    start = text.index("fn control_request_for")
    end = text.index("\n}\n", start)
    return set(re.findall(r"GodotCommand::([A-Za-z0-9]+)", text[start:end]))


def handled_requests():
    """ControlRequest variants the render loop acts on."""
    text = MAIN.read_text(encoding="utf-8", errors="replace")
    return set(re.findall(r"ControlRequest::([A-Za-z0-9]+)", text))


def answered_by_socket():
    """Commands answered directly by the IPC server, needing no loop handler."""
    text = IPC.read_text(encoding="utf-8")
    start = text.index("async fn process_command")
    end = text.index("\npub struct IpcClient", start)
    return set(re.findall(r"GodotCommand::([A-Za-z0-9]+)", text[start:end]))


def send(command, timeout=15):
    payload = json.dumps(command).encode()
    with socket.create_connection(ADDR, timeout=timeout) as sock:
        sock.sendall(struct.pack(">I", len(payload)) + payload)
        header = b""
        while len(header) < 4:
            header += sock.recv(4 - len(header))
        length = struct.unpack(">I", header)[0]
        body = b""
        while len(body) < length:
            body += sock.recv(length - len(body))
    return json.loads(body.decode())


def live_check():
    """Probe the running app for commands that are accepted but do nothing.

    A command queued to a loop that ignores it returns success, so a status
    command is used that only the loop can answer.
    """
    candidates = [
        ROOT / "target" / "release" / "screenbuddy.exe",
        ROOT / "target" / "debug" / "screenbuddy.exe",
    ]
    existing = [p for p in candidates if p.exists()]
    if not existing:
        return None

    exe = max(existing, key=lambda p: p.stat().st_mtime)
    app = subprocess.Popen([str(exe)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        for _ in range(60):
            try:
                socket.create_connection(ADDR, timeout=1).close()
                break
            except OSError:
                time.sleep(0.5)
        else:
            return None
        time.sleep(1.5)
        return send({"cmd": "get_status"})
    finally:
        app.terminate()
        try:
            app.wait(timeout=10)
        except subprocess.TimeoutExpired:
            app.kill()


def main():
    tools = mcp_tools()
    declared = ipc_commands()
    mapped = mapped_commands()
    answered = answered_by_socket()
    handled = handled_requests()

    print(f"{len(tools)} MCP tools, {len(declared)} IPC commands\n")

    # 1. MCP tools must target a command the IPC layer knows.
    unknown = {t: c for t, c in tools.items() if pascal(c) not in declared}
    print(f"MCP tools sending an undeclared command: {len(unknown)}")
    for t, c in sorted(unknown.items()):
        print(f"  {t} -> '{c}'")
    if not unknown:
        print("  (none)")

    # 2. Every command must be reachable: handled by the loop or answered inline.
    unreachable = declared - mapped - answered
    print(f"\nCommands declared but neither mapped nor answered: {len(unreachable)}")
    for name in sorted(unreachable):
        print(f"  {name}")
    if not unreachable:
        print("  (none)")

    # 3. A mapped command the loop never handles is accepted then dropped.
    dropped = mapped - handled
    print(f"\nMapped commands the render loop never handles: {len(dropped)}")
    for name in sorted(dropped):
        print(f"  {name}")
    if not dropped:
        print("  (none)")

    # 4. The app must actually be running and answering.
    status = live_check()
    alive = isinstance(status, dict) and status.get("status") == "success"
    print(f"\nApp responds over IPC: {'yes' if alive else 'no'}")

    # Full picture, so a clean result can be eyeballed for parser mistakes.
    reachable = declared & (mapped | answered)
    print(f"\nReachable commands: {len(reachable)}/{len(declared)}")
    print("  queued to the loop: " + ", ".join(sorted(mapped)))
    print("  answered by the socket: " + ", ".join(sorted(answered)))
    print("  handled by the loop: " + ", ".join(sorted(handled)))

    ok = not unknown and not unreachable and not dropped and alive
    print(f"\n{'PASS' if ok else 'FAIL'} - mcp/ipc audit")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())