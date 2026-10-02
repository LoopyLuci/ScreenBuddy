#!/usr/bin/env python3
"""MCP server exposing a running ScreenBuddy instance to an agent.

Speaks the Model Context Protocol over stdio and forwards tool calls to the
app's IPC control surface (length-prefixed JSON over TCP). Hermes discovers these
tools at startup and registers them as `mcp_screenbuddy_*`.

Requires the ScreenBuddy app to be running; it starts its IPC server on
127.0.0.1:34567. Override with SCREENBUDDY_IPC_ADDR.

Usage:
    python mcp_server.py            # stdio transport (for Hermes)
"""

from __future__ import annotations

import json
import os
import socket
import struct
import sys
import time
from typing import Any

try:  # mcp >= 2.0 renamed FastMCP to MCPServer
    from mcp.server.mcpserver import MCPServer as _Server
except ModuleNotFoundError:  # mcp 1.x
    from mcp.server.fastmcp import FastMCP as _Server

DEFAULT_ADDR = os.environ.get("SCREENBUDDY_IPC_ADDR", "127.0.0.1:34567")
DEFAULT_TIMEOUT = float(os.environ.get("SCREENBUDDY_IPC_TIMEOUT", "10"))

mcp = _Server(
    "screenbuddy",
    instructions=(
        "Control a running ScreenBuddy desktop companion: query status, move and "
        "animate creatures, talk to its AI, manage settings, and use its memory. "
        "Call screenbuddy_ping or screenbuddy_status first to confirm the app is up."
    ),
)


# --------------------------------------------------------------------------
# IPC transport
# --------------------------------------------------------------------------


class ScreenBuddyError(RuntimeError):
    """Raised when the app is unreachable or rejects a command."""


def _read_exactly(sock: socket.socket, count: int) -> bytes:
    chunks = b""
    while len(chunks) < count:
        block = sock.recv(count - len(chunks))
        if not block:
            raise ScreenBuddyError("connection closed mid-frame")
        chunks += block
    return chunks


def send_command(command: dict[str, Any], addr: str = DEFAULT_ADDR,
                 timeout: float = DEFAULT_TIMEOUT) -> dict[str, Any]:
    """Send one IPC frame and return the decoded response envelope."""
    host, _, port = addr.rpartition(":")
    payload = json.dumps(command).encode("utf-8")
    try:
        with socket.create_connection((host, int(port)), timeout=timeout) as sock:
            sock.settimeout(timeout)
            sock.sendall(struct.pack(">I", len(payload)) + payload)
            header = _read_exactly(sock, 4)
            (length,) = struct.unpack(">I", header)
            if length > 8 * 1024 * 1024:
                raise ScreenBuddyError(f"response of {length} bytes is implausible")
            body = _read_exactly(sock, length)
    except ConnectionRefusedError as exc:
        raise ScreenBuddyError(
            f"ScreenBuddy is not running (nothing listening on {addr}). "
            "Launch the app first."
        ) from exc
    except OSError as exc:
        raise ScreenBuddyError(f"could not reach ScreenBuddy at {addr}: {exc}") from exc

    return json.loads(body.decode("utf-8"))


def call(command: dict[str, Any], addr: str = DEFAULT_ADDR,
         timeout: float = DEFAULT_TIMEOUT) -> Any:
    """Send a command and unwrap the success payload, raising on error."""
    response = send_command(command, addr, timeout)
    status = response.get("status")
    if status == "error":
        raise ScreenBuddyError(response.get("message", "unknown IPC error"))
    return response.get("data")


def wait_for_app(addr: str = DEFAULT_ADDR, attempts: int = 20,
                 delay: float = 0.5) -> bool:
    """Poll until the app answers, so a just-launched instance is found."""
    for _ in range(attempts):
        try:
            if send_command({"cmd": "ping"}, addr, 2.0).get("status") == "pong":
                return True
        except ScreenBuddyError:
            pass
        time.sleep(delay)
    return False


# --------------------------------------------------------------------------
# Tools
# --------------------------------------------------------------------------


@mcp.tool()
def screenbuddy_status() -> dict[str, Any]:
    """Live runtime snapshot: fps, uptime, creature count, AI models, memory size.

    Use this first to confirm the app is running and healthy.
    """
    return call({"cmd": "get_status"})


@mcp.tool()
def screenbuddy_ping() -> bool:
    """Check that the ScreenBuddy app is reachable.

    Returns True on success; raises if the app is not running.
    """
    return send_command({"cmd": "ping"}).get("status") == "pong"


@mcp.tool()
def screenbuddy_list_creatures() -> dict[str, Any]:
    """List every loaded creature with its position, visibility and animation state."""
    return call({"cmd": "list_creature_state"})


@mcp.tool()
def screenbuddy_set_animation(id: str, state: str) -> str:
    """Set the animation state of one creature, or of all when `id` is omitted.

    Args:
        id: Creature id (e.g. "companion-bird-01") or empty string for all.
        state: One of idle, walk, fly, sleep, celebrate.
    """
    valid = {"idle", "walk", "fly", "sleep", "celebrate"}
    if state not in valid:
        raise ValueError(f"state must be one of {sorted(valid)}, got {state!r}")
    call({"cmd": "set_animation", "id": id or None, "state": state})
    return f"animation set to {state}" + (f" for {id}" if id else " for all creatures")


@mcp.tool()
def screenbuddy_move_creature(id: str, x: float, y: float) -> str:
    """Move a creature to an absolute screen position and pin it there.

    Args:
        id: Creature id, e.g. "companion-bird-01".
        x: Screen x coordinate in pixels.
        y: Screen y coordinate in pixels.
    """
    call({"cmd": "move_creature", "id": id, "x": float(x), "y": float(y)})
    return f"moved {id} to ({x}, {y})"


@mcp.tool()
def screenbuddy_set_visible(id: str, visible: bool) -> str:
    """Show or hide a creature.

    Args:
        id: Creature id, e.g. "companion-bird-01".
        visible: True to show, False to hide.
    """
    call({"cmd": "set_creature_visible", "id": id, "visible": bool(visible)})
    return f"{id} {'shown' if visible else 'hidden'}"


@mcp.tool()
def screenbuddy_set_auto_cycle(enabled: bool) -> str:
    """Enable or disable the automatic animation rotation.

    Turn it off to hold a specific animation state set via screenbuddy_set_animation.
    """
    call({"cmd": "set_auto_cycle", "enabled": bool(enabled)})
    return f"auto cycle {'enabled' if enabled else 'disabled'}"


@mcp.tool()
def screenbuddy_send_chat(message: str) -> str:
    """Send a message to ScreenBuddy's AI chat and return the queued result.

    The reply appears in the app's chat window; this returns once the app
    accepts the message.
    """
    call({"cmd": "send_chat", "message": message})
    return "chat message sent"


@mcp.tool()
def screenbuddy_chat_history() -> dict[str, Any]:
    """The current chat transcript from the app."""
    return call({"cmd": "get_chat_history"})


@mcp.tool()
def screenbuddy_run_agent(prompt: str) -> str:
    """Run a prompt through the app's agent runtime with tools enabled.

    Args:
        prompt: Instruction for the agent, e.g. "summarise the active creatures".
    """
    call({"cmd": "run_agent", "prompt": prompt})
    return "agent prompt queued"


@mcp.tool()
def screenbuddy_agent_info() -> dict[str, Any]:
    """Registered agent tools and the last tool call / response."""
    return call({"cmd": "get_agent_info"})


@mcp.tool()
def screenbuddy_play_sound(name: str) -> str:
    """Play a sound effect.

    Args:
        name: One of the bundled effects: click, notify, celebrate, idle_hum,
             walk_step, snore.
    """
    call({"cmd": "play_sound", "name": name})
    return f"played {name}"


@mcp.tool()
def screenbuddy_speak(text: str) -> str:
    """Speak text aloud using the app's text-to-speech engine."""
    call({"cmd": "speak", "text": text})
    return "speaking"


@mcp.tool()
def screenbuddy_set_setting(key: str, value: Any) -> str:
    """Write a runtime setting.

    Args:
        key: Setting name, e.g. "volume_master" or "animation_speed".
        value: Number, boolean or string.
    """
    call({"cmd": "set_setting", "key": key, "value": value})
    return f"set {key} = {value}"


@mcp.tool()
def screenbuddy_get_setting(key: str = "") -> dict[str, Any]:
    """Read one setting, or all of them when `key` is empty.

    Args:
        key: Setting name, or empty for every setting.
    """
    return call({"cmd": "get_setting", "key": key or None})


@mcp.tool()
def screenbuddy_memory_ingest(source: str, content: str) -> str:
    """Add a document to the app's RAG memory so it can be recalled later.

    Args:
        source: Identifier for where the text came from, e.g. "notes://project".
        content: The text to remember.
    """
    call({"cmd": "memory_ingest", "source": source, "content": content})
    return f"ingested from {source}"


@mcp.tool()
def screenbuddy_memory_search(query: str, limit: int = 5) -> dict[str, Any]:
    """Search RAG memory and return the matching passages.

    Args:
        query: What to look for.
        limit: Maximum passages to return (1-50).
    """
    call({"cmd": "memory_search", "query": query, "limit": int(limit)})
    # Retrieval is queued and applied by the render loop, then published to the
    # snapshot; poll briefly so the results reflect this query and not an older one.
    for _ in range(10):
        time.sleep(0.15)
        results = call({"cmd": "get_search_results"})
        if results.get("query") == query:
            return results
    return call({"cmd": "get_search_results"})


@mcp.tool()
def screenbuddy_list_sounds() -> list[str]:
    """Names of the bundled sound effects, for use with screenbuddy_play_sound."""
    return ["click", "notify", "celebrate", "idle_hum", "walk_step", "snore"]


def main() -> int:
    # Probe once, briefly. Blocking here would stall Hermes' startup discovery,
    # so a missing app must not delay registering the tools: the server comes up
    # either way and each tool reports the connection error when called.
    if not wait_for_app(attempts=2, delay=0.2):
        print(
            f"ScreenBuddy not reachable at {DEFAULT_ADDR}; tools will error until it starts.",
            file=sys.stderr,
        )
    # 2.x requires an explicit transport; 1.x defaults to stdio.
    try:
        mcp.run(transport="stdio")
    except TypeError:
        mcp.run()
    return 0


if __name__ == "__main__":
    sys.exit(main())