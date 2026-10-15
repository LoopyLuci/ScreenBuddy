"""ScreenBuddy as an AgenticBotPlatform backend kind.

A bot routed to `backend: screenbuddy` is answered by the ScreenBuddy desktop
companion: the creature's own persona, memory, animations and sounds reply,
while ABP keeps everything around it — channels, the learned router, backup
chains, swarms, schedules. ABP's own permission rules and tool loop do not
apply inside ScreenBuddy, the same documented trade as the opencode/openclaw
backends.

Each turn spawns one `tools/abp_client.py` process (the external-agent
pattern from bot/backends/external_agent_backend.py): it sends the prompt over
ScreenBuddy's IPC surface, polls the chat history for the creature's reply, and
prints it. The process runs in its own cell — a Win32 Job Object on Windows —
so a timeout kills the tree, and one turn per process keeps ScreenBuddy's own
single-threaded chat loop untouched.

Registered with zero ABP core edits via api.add_backend_class. Activate by
pointing ABP_PRIVATE_DIR at the folder holding extensions/ (see the README in
ScreenBuddy's integrations/abp/), then set a bot instance's backend to
`screenbuddy`.
"""
from __future__ import annotations

import asyncio
import os
import shutil
import socket
from typing import Any, Callable, Optional

from bot.backends.base import Backend, BackendError, BackendResult, process_cell
from bot.sandbox_ns.spawn import async_spawn

DEFAULT_CLIENT = "python"
DEFAULT_SCRIPT = r"C:\Projects\ScreenBuddy\tools\abp_client.py"
DEFAULT_ADDR = "127.0.0.1:34567"
DEFAULT_TIMEOUT_S = 300.0


class ScreenBuddyBackend(Backend):
    """One turn = one abp_client.py process against a running ScreenBuddy."""

    name = "screenbuddy"
    preset = "external"

    def __init__(self, settings: dict, model: Optional[str] = None):
        s = settings or {}
        self.client = str(s.get("client") or DEFAULT_CLIENT)
        self.script = str(s.get("script") or DEFAULT_SCRIPT)
        self.addr = str(s.get("addr") or DEFAULT_ADDR)
        self.timeout_s = float(s.get("timeout_s") or DEFAULT_TIMEOUT_S)
        self.model = model

    async def ask(self, prompt: str, *, context: Optional[dict] = None,
                  timeout_s: float = 300) -> BackendResult:
        # The instance's configured timeout wins over the argument default:
        # a small local model through the creature takes 40-60s, and the
        # generic 30s router default would kill every real turn.
        timeout_s = max(float(timeout_s), self.timeout_s) if self.timeout_s else float(timeout_s)

        exe = shutil.which(self.client) or (self.client if os.path.isfile(self.client) else None)
        if exe is None:
            raise BackendError(
                f"{self.client!r} was not found on PATH - it runs ScreenBuddy's tools/abp_client.py"
            )
        if not os.path.isfile(self.script):
            raise BackendError(
                f"ScreenBuddy client script not found at {self.script!r} "
                f"(set backends.screenbuddy.script in the config overlay)"
            )

        args = [exe, self.script, "--addr", self.addr, "--timeout",
                f"{timeout_s:g}", prompt]
        kwargs: dict[str, Any] = {}
        if os.name == "nt":
            import subprocess

            kwargs["creationflags"] = subprocess.CREATE_NEW_PROCESS_GROUP

        # One cell per turn, so the tree kill below is the cell's own and the
        # run shows on the diagnostics page.
        cell = process_cell(f"{self.name} {self.script}", owner="backends.screenbuddy")
        try:
            proc = await async_spawn(
                args, cell=cell, stdin=asyncio.subprocess.DEVNULL,
                stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE,
                name=f"{self.name} abp_client", owner="backends.screenbuddy", **kwargs,
            )
        except OSError as exc:
            cell.close()
            raise BackendError(f"could not start ScreenBuddy client: {exc}") from exc
        try:
            try:
                out, err = await asyncio.wait_for(proc.communicate(), timeout=timeout_s + 15)
            except asyncio.TimeoutError:
                await _kill_tree(proc)
                raise BackendError(f"ScreenBuddy timed out after {timeout_s:g}s") from None
            except asyncio.CancelledError:
                await _kill_tree(proc)
                raise
            text_out = out.decode("utf-8", "replace").strip()
            if proc.returncode != 0:
                detail = err.decode("utf-8", "replace").strip() or text_out
                raise BackendError(
                    f"ScreenBuddy client exited with status {proc.returncode}: {detail[:500]}"
                )
            if not text_out:
                raise BackendError("ScreenBuddy returned no text")
            return BackendResult(
                text=text_out,
                raw={"backend": self.name, "exit": proc.returncode},
            )
        finally:
            cell.close()


async def _kill_tree(proc) -> None:
    from bot.backends.external_agent_backend import _kill_tree as kill

    await kill(proc)


def ready() -> tuple[bool, str]:
    """Pre-flight: is a ScreenBuddy actually listening on its IPC port?

    The router runs this before each attempt, so an unrunned companion is
    reported as 'not set up' and the backup chain moves on, rather than
    every turn spawning a client that cannot connect.
    """
    addr = os.environ.get("SCREENBUDDY_IPC_ADDR") or DEFAULT_ADDR
    host, _, port = addr.rpartition(":")
    try:
        with socket.create_connection((host or "127.0.0.1", int(port)), timeout=2.0):
            return True, ""
    except OSError:
        return False, f"ScreenBuddy is not running on {addr} (start the desktop app)"


def factory(settings: dict, model: Optional[str] = None) -> ScreenBuddyBackend:
    """The factory add_backend_class expects: (backends.screenbuddy, model) -> Backend."""
    return ScreenBuddyBackend(settings, model)


def register(api) -> None:
    api.add_backend_class(
        "screenbuddy",
        factory,
        family="external",
        label="ScreenBuddy (desktop companion)",
        ready=ready,
    )
