"""Pattern L: one tool, one bounded cell.

The creature's agent routes anything dangerous (shelling out, writing
files, fetching URLs) to `POST /api/screenbuddy/sandbox_exec`, which
lands here. The worker (sandbox_worker.py) runs in a `process_cell` — a
Windows Job Object on this host — so the tool executes under CPU, memory
and wall-clock limits, and one kill takes the whole tree.

The dashboard's own process is never the thing a runaway tool can take
down: the worker is a child, the cell bounds it, and the route's timeout
kills the tree and says so.
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path
from typing import Optional

MAX_CHARS = 8000
DEFAULT_TIMEOUT_S = 60.0


def _worker_path() -> str:
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), "sandbox_worker.py")


def execute(tool: str, args: dict, timeout_s: Optional[float] = None) -> dict:
    """Run `tool` in a cell; return its bounded output.

    Every outcome is reported honestly: the tool's output (truncated to
    MAX_CHARS), an error string from the worker, or a timeout that killed
    the tree — `timed_out` is always stated, never implied.
    """
    from bot.backends.base import process_cell
    from bot.sandbox_ns import spawn

    timeout = float(timeout_s or DEFAULT_TIMEOUT_S)
    cell = process_cell("screenbuddy-sandbox", preset="tool", owner="screenbuddy")
    try:
        # The worker imports bot.*, so the ABP root must be on its
        # PYTHONPATH — the route passes the running process's own root.
        import bot

        abp_root = str(Path(bot.__file__).resolve().parent.parent)
        env = dict(os.environ)
        env["PYTHONPATH"] = abp_root + os.pathsep + env.get("PYTHONPATH", "")
        # spawn.run already captures stdout/stderr as text and kills the
        # cell's whole tree itself when the timeout fires.
        proc = spawn.run(
            [sys.executable, _worker_path(), tool, json.dumps(args or {})],
            timeout=timeout,
            cell=cell,
            env=env,
        )
    except subprocess.TimeoutExpired as exc:
        partial = exc.stdout or ""
        if isinstance(partial, bytes):
            partial = partial.decode("utf-8", "replace")
        return {
            "ok": False,
            "timed_out": True,
            "output": partial[-MAX_CHARS:],
            "error": f"timed out after {timeout:.0f}s — the cell was killed",
        }
    except Exception as exc:  # noqa: BLE001 - a spawn failure is a result too
        return {"ok": False, "timed_out": False, "output": "",
                "error": f"sandbox spawn failed: {exc}"}
    finally:
        cell.close()

    output = (proc.stdout or "")[-MAX_CHARS:]
    stderr = (proc.stderr or "").strip()
    if proc.returncode != 0 and stderr and not output:
        output = f"error: {stderr[:MAX_CHARS]}"
    return {
        "ok": proc.returncode == 0,
        "timed_out": False,
        "output": output,
        "truncated": len(proc.stdout or "") > MAX_CHARS,
    }
