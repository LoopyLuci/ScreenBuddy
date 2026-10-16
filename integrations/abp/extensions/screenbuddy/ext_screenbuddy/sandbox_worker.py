"""Run one tool inside a sandbox cell. Executed by the extension's
sandbox_exec route, never directly.

The route spawns this file in a `process_cell` (a Windows Job Object on
this host): bounded CPU, memory and wall-clock, and one kill takes the
whole tree. The tool runs through the same dispatch the agent runtime
uses (`bot.agent_runtime.tools.execute_tool`), so a dangerous tool the
creature asked for executes under the cell's limits instead of in the
dashboard's own process.

Tools that are MCP-routed (they need a live server connection) cannot run
here; the route reports that honestly rather than pretending.
"""
from __future__ import annotations

import asyncio
import json
import sys
import tempfile
from pathlib import Path

MAX_CHARS = 8000


def main() -> int:
    if len(sys.argv) < 3:
        print("error: usage: sandbox_worker.py <tool> <json-args>", file=sys.stderr)
        return 2
    tool, raw_args = sys.argv[1], sys.argv[2]
    try:
        args = json.loads(raw_args or "{}")
    except ValueError as exc:
        print(f"error: bad args json: {exc}", file=sys.stderr)
        return 2

    from bot.agent_runtime import tools as tools_mod

    workspace = Path(tempfile.mkdtemp(prefix="sb_sandbox_"))
    try:
        out = asyncio.run(tools_mod.execute_tool(tool, args, workspace=workspace))
    except Exception as exc:  # noqa: BLE001 - a failing tool is a result, not a crash
        out = f"error: {exc}"
    text = str(out)
    if len(text) > MAX_CHARS:
        text = text[:MAX_CHARS] + "\n…[truncated]"
    print(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
