#!/usr/bin/env python3
"""Drive the ScreenBuddy MCP server over stdio and verify its tools.

Acts as a minimal MCP client: performs the initialize handshake, lists tools,
and calls a representative subset against a running ScreenBuddy app.

Usage:
    python mcp/test_mcp.py            # requires the app to be running
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import threading

HERE = os.path.dirname(os.path.abspath(__file__))
SERVER = os.path.join(HERE, "server.py")
PYTHON = sys.executable


class McpStdioClient:
    """Minimal MCP stdio client: newline-delimited JSON-RPC over pipes."""

    def __init__(self) -> None:
        self.proc = subprocess.Popen(
            [PYTHON, SERVER],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        self._id = 0
        self._stderr: list[str] = []
        threading.Thread(target=self._drain_stderr, daemon=True).start()

    def _drain_stderr(self) -> None:
        for line in self.proc.stderr:
            self._stderr.append(line.rstrip())

    def request(self, method: str, params: dict | None = None) -> dict:
        self._id += 1
        message = {"jsonrpc": "2.0", "id": self._id, "method": method}
        if params is not None:
            message["params"] = params
        self.proc.stdin.write(json.dumps(message) + "\n")
        self.proc.stdin.flush()
        while True:
            line = self.proc.stdout.readline()
            if not line:
                raise RuntimeError(f"server closed; stderr={self._stderr}")
            try:
                response = json.loads(line)
            except json.JSONDecodeError:
                continue
            if response.get("id") == self._id:
                if "error" in response:
                    raise RuntimeError(f"{method} failed: {response['error']}")
                return response.get("result", {})

    def notify(self, method: str, params: dict | None = None) -> None:
        message = {"jsonrpc": "2.0", "method": method}
        if params is not None:
            message["params"] = params
        self.proc.stdin.write(json.dumps(message) + "\n")
        self.proc.stdin.flush()

    def call_tool(self, name: str, args: dict | None = None) -> dict:
        result = self.request(
            "tools/call", {"name": name, "arguments": args or {}}
        )
        if result.get("isError"):
            text = " ".join(
                c.get("text", "") for c in result.get("content", []) if c.get("type") == "text"
            )
            raise RuntimeError(f"{name} errored: {text}")
        return result

    def close(self) -> None:
        try:
            self.proc.stdin.close()
        except Exception:
            pass
        self.proc.terminate()
        try:
            self.proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.proc.kill()


def parse_tool_text(result: dict) -> object:
    """Decode the payload an MCP tool returned.

    Tools return dicts/lists which the framework serialises as JSON text, but a
    bare list may arrive as a Python repr, so fall back to ast.literal_eval.
    """
    texts = [
        block["text"] for block in result.get("content", []) if block.get("type") == "text"
    ]
    if not texts:
        return None
    raw = texts[0]
    try:
        return json.loads(raw)
    except json.JSONDecodeError:
        pass
    # A bare list may arrive as one block per element, or as a Python repr.
    joined = "".join(texts)
    try:
        import ast

        return ast.literal_eval(joined)
    except (ValueError, SyntaxError):
        return joined


def check_cold_start() -> tuple[str, str, str]:
    """Verify the server can start the app itself when nothing is listening.

    Skipped when autostart is disabled or the app cannot be found, so the suite
    still runs on a machine without a built binary.
    """
    import os
    import socket
    import subprocess
    import sys
    import time

    if os.environ.get("SCREENBUDDY_AUTOSTART", "1") in ("0", "false", "no"):
        return ("cold start", "SKIP", "autostart disabled")

    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    try:
        import server as sb
    except Exception as exc:  # noqa: BLE001
        return ("cold start", "SKIP", str(exc)[:80])

    exe = sb.find_app()
    if exe is None:
        return ("cold start", "SKIP", "no executable found")

    addr = os.environ.get("SCREENBUDDY_IPC_ADDR", "127.0.0.1:34567")
    host, _, port = addr.rpartition(":")

    def port_open() -> bool:
        s = socket.socket()
        s.settimeout(1)
        try:
            s.connect((host, int(port)))
            return True
        except OSError:
            return False
        finally:
            s.close()

    subprocess.run(["taskkill", "/F", "/IM", exe.name], capture_output=True, text=True)
    for _ in range(20):
        if not port_open():
            break
        time.sleep(0.5)
    if port_open():
        return ("cold start", "SKIP", "could not stop the running app")

    proc = subprocess.Popen(
        [sys.executable, os.path.join(os.path.dirname(os.path.abspath(__file__)), "server.py")],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        text=True, bufsize=1,
    )
    try:
        req_id = 0

        def req(method, params=None):
            nonlocal req_id
            req_id += 1
            msg = {"jsonrpc": "2.0", "id": req_id, "method": method}
            if params:
                msg["params"] = params
            proc.stdin.write(json.dumps(msg) + "\n")
            proc.stdin.flush()
            while True:
                line = proc.stdout.readline()
                if not line:
                    raise RuntimeError("server closed")
                try:
                    r = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if r.get("id") == req_id:
                    if "error" in r:
                        raise RuntimeError(r["error"])
                    return r.get("result", {})

        req("initialize", {"protocolVersion": "2024-11-05", "capabilities": {},
                           "clientInfo": {"name": "coldstart", "version": "1"}})
        proc.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
        proc.stdin.flush()
        req("tools/list")
        if not port_open():
            raise AssertionError("server started but the app never opened its port")
        return ("cold start", "PASS", "server launched the app")
    except Exception as exc:  # noqa: BLE001
        return ("cold start", "FAIL", str(exc)[:120])
    finally:
        proc.stdin.close()
        proc.terminate()


def main() -> int:
    client = McpStdioClient()
    checks: list[tuple[str, str, str]] = []

    def check(name: str, fn) -> None:
        try:
            fn()
            checks.append((name, "PASS", ""))
        except Exception as exc:  # noqa: BLE001 - report all failures
            checks.append((name, "FAIL", str(exc)[:160]))

    try:
        init = client.request(
            "initialize",
            {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "screenbuddy-mcp-test", "version": "1.0"},
            },
        )
        server_info = init.get("serverInfo", {})
        print(f"connected to: {server_info.get('name')} {server_info.get('version')}")
        client.notify("notifications/initialized")

        tools = client.request("tools/list")["tools"]
        names = {t["name"] for t in tools}
        print(f"tools discovered: {len(names)}\n")

        def t_lists_expected_tools():
            expected = {
                "screenbuddy_ping",
                "screenbuddy_status",
                "screenbuddy_list_creatures",
                "screenbuddy_set_animation",
                "screenbuddy_move_creature",
                "screenbuddy_set_visible",
                "screenbuddy_set_auto_cycle",
                "screenbuddy_send_chat",
                "screenbuddy_chat_history",
                "screenbuddy_run_agent",
                "screenbuddy_agent_info",
                "screenbuddy_play_sound",
                "screenbuddy_speak",
                "screenbuddy_set_setting",
                "screenbuddy_get_setting",
                "screenbuddy_memory_ingest",
                "screenbuddy_memory_search",
                "screenbuddy_list_sounds",
            }
            missing = expected - names
            assert not missing, f"missing tools: {sorted(missing)}"

        def t_ping():
            assert parse_tool_text(client.call_tool("screenbuddy_ping")) is True

        def t_status():
            data = parse_tool_text(client.call_tool("screenbuddy_status"))
            assert data["creature_count"] > 0, data
            assert data["frames"] > 0, data
            assert data["fps"] > 1.0, data

        def t_list_creatures():
            data = parse_tool_text(client.call_tool("screenbuddy_list_creatures"))
            assert data["creatures"], data
            bird = data["creatures"].get("companion-bird-01")
            assert bird and "state" in bird, data

        def t_animation_validation():
            try:
                client.call_tool(
                    "screenbuddy_set_animation",
                    {"id": "companion-bird-01", "state": "bogus"},
                )
            except RuntimeError:
                return  # rejected as expected
            raise AssertionError("invalid animation state was accepted")

        def t_set_animation():
            client.call_tool("screenbuddy_set_auto_cycle", {"enabled": False})
            client.call_tool(
                "screenbuddy_set_animation",
                {"id": "companion-bird-01", "state": "celebrate"},
            )
            import time

            time.sleep(0.8)
            data = parse_tool_text(client.call_tool("screenbuddy_list_creatures"))
            assert data["creatures"]["companion-bird-01"]["state"] == "celebrate", data

        def t_move_and_visible():
            client.call_tool(
                "screenbuddy_move_creature",
                {"id": "companion-bird-01", "x": 400.0, "y": 250.0},
            )
            client.call_tool(
                "screenbuddy_set_visible",
                {"id": "companion-bird-01", "visible": False},
            )
            import time

            time.sleep(0.8)
            bird = parse_tool_text(client.call_tool("screenbuddy_list_creatures"))["creatures"][
                "companion-bird-01"
            ]
            assert abs(bird["x"] - 400.0) < 1.0, bird
            assert abs(bird["y"] - 250.0) < 1.0, bird
            assert bird["visible"] is False, bird
            # restore
            client.call_tool(
                "screenbuddy_set_visible", {"id": "companion-bird-01", "visible": True}
            )

        def t_setting_roundtrip():
            client.call_tool(
                "screenbuddy_set_setting", {"key": "volume_master", "value": 0.33}
            )
            import time

            time.sleep(0.5)
            data = parse_tool_text(
                client.call_tool("screenbuddy_get_setting", {"key": "volume_master"})
            )
            assert abs(float(data["value"]) - 0.33) < 0.001, data

        def t_list_sounds():
            sounds = parse_tool_text(client.call_tool("screenbuddy_list_sounds"))
            assert "celebrate" in sounds, sounds

        def t_memory_roundtrip():
            client.call_tool(
                "screenbuddy_memory_ingest",
                {
                    "source": "probe://birds",
                    "content": "the albatross can glide for hours without flapping",
                },
            )
            import time

            time.sleep(0.6)
            data = parse_tool_text(
                client.call_tool(
                    "screenbuddy_memory_search", {"query": "albatross glide", "limit": 3}
                )
            )
            assert data["query"] == "albatross glide", data
            assert data["results"], f"expected a match, got {data}"
            assert "albatross" in data["results"][0]["text"], data

        def t_chat_history_shape():
            data = parse_tool_text(client.call_tool("screenbuddy_chat_history"))
            assert "messages" in data, data

        def t_agent_info():
            data = parse_tool_text(client.call_tool("screenbuddy_agent_info"))
            assert data["tool_count"] > 0, data

        for name, fn in [
            ("all expected tools registered", t_lists_expected_tools),
            ("ping", t_ping),
            ("status reports live runtime", t_status),
            ("list creatures", t_list_creatures),
            ("invalid animation state rejected", t_animation_validation),
            ("set animation takes effect", t_set_animation),
            ("move + visibility take effect", t_move_and_visible),
            ("setting write then read", t_setting_roundtrip),
            ("list sounds", t_list_sounds),
            ("memory ingest then search", t_memory_roundtrip),
            ("chat history shape", t_chat_history_shape),
            ("agent info", t_agent_info),
        ]:
            check(name, fn)

        # Leave the app as we found it.
        try:
            client.call_tool("screenbuddy_set_auto_cycle", {"enabled": True})
            client.call_tool(
                "screenbuddy_set_animation", {"id": "companion-bird-01", "state": "idle"}
            )
        except Exception:
            pass

    finally:
        client.close()

    name, result, detail = check_cold_start()
    checks.append((name, result, detail))

    width = max(len(n) for n, _, _ in checks)
    for name, result, detail in checks:
        suffix = f"  {detail}" if detail else ""
        print(f"  {name:<{width}}  {result}{suffix}")
    passed = [n for n, r, _ in checks if r == "PASS"]
    skipped = [(n, d) for n, r, d in checks if r == "SKIP"]
    failed = [n for n, r, _ in checks if r == "FAIL"]

    for name, detail in skipped:
        print(f"  {name:<{width}}  SKIP ({detail})")
    print(f"\n{len(passed)}/{len(checks) - len(skipped)} MCP checks passed")
    if skipped:
        print(f"{len(skipped)} skipped")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())