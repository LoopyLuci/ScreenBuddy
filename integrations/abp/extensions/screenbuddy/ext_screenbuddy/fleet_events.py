"""Fleet events, made visible: jobs → board → creature.

A background task (add_task) polls ABP's own jobs table for terminal
transitions — a job that finished succeeded or failed — and does two
things with each: writes the fleet board (a shared_context doc every bot
can read on any turn) and reacts on the creature (Celebrate + a spoken
line, opt-in). It also keeps a creature-status doc current, so a bot can
answer "how is the creature doing?" without any IPC of its own.

The loop hazard from Pattern D applies here in a new form: a reaction
must be terminal — celebrating a job must never re-run the job. Nothing
in this module sends a prompt to a bot, only to the creature's animation
and voice, so the loop cannot close.

State (the watermark and announced ids) lives in a small JSON file next
to this module, so an ABP restart re-announces nothing.
"""
from __future__ import annotations

import asyncio
import hashlib
import json
import os
from typing import Any, Optional

from . import ipc

BOARD_DOC = "fleet-board"
CREATURE_DOC = "creature-status"
DEFAULT_INTERVAL_S = 10.0
MAX_BOARD_ENTRIES = 20


# ---- the pure part: what counts as new and terminal ----------------------

def terminal_jobs(conn, watermark: Optional[str], limit: int = 100) -> list[dict]:
    """Jobs that reached a terminal state after the watermark, oldest first.
    Terminal = success or failed; queued/running/retrying are not news yet."""
    cols = ("id, action_type, backend, status, error, finished_at, "
            "instance_id, swarm_run_id")
    if watermark:
        rows = conn.execute(
            f"SELECT {cols} FROM jobs "
            "WHERE status IN ('success', 'failed') AND finished_at > ? "
            "ORDER BY finished_at ASC LIMIT ?",
            (watermark, limit),
        ).fetchall()
    else:
        rows = conn.execute(
            f"SELECT {cols} FROM jobs "
            "WHERE status IN ('success', 'failed') AND finished_at IS NOT NULL "
            "ORDER BY finished_at ASC LIMIT ?",
            (limit,),
        ).fetchall()
    return [_job_dict(r) for r in rows]


def _job_dict(row) -> dict:
    base = {
        "id": row[0], "action": row[1], "backend": row[2], "status": row[3],
        "error": row[4], "finished_at": row[5],
    }
    # instance_id and swarm_run_id exist in the current schema; older rows
    # (and test fixtures) may not carry them.
    try:
        base["instance_id"] = row[6]
        base["swarm_run_id"] = row[7]
    except (IndexError, TypeError):
        pass
    return base


def instance_addrs(swarm: list, default_addr: str) -> dict:
    """job.instance_id -> the IPC addr of that instance's creature (Pattern M).

    A swarm member instance's backend field is the member's registered kind
    name (screenbuddy-2, ...), so the mapping is exact: backend -> addr.
    The primary kind resolves to the default addr. Anything unknown resolves
    to None and reacts on the primary, which is always safe.
    """
    from bot import bot_instances

    addrs: dict = {}
    try:
        for inst in bot_instances.list_instances():
            backend = str(inst.get("backend") or "").strip()
            if not backend:
                continue
            if backend == "screenbuddy":
                addrs[int(inst["id"])] = default_addr
                continue
            for member in swarm or []:
                if str(member.get("name") or "") == backend:
                    addrs[int(inst["id"])] = str(member.get("addr") or "")
                    break
    except Exception:  # noqa: BLE001 - addr resolution must never kill the tick
        return addrs
    return addrs


def board_content(jobs: list[dict]) -> str:
    """The fleet board as markdown: the latest completions, newest last.
    Kept small on purpose — the board is meant to be read constantly."""
    lines = ["# Fleet board", ""]
    recent = jobs[-MAX_BOARD_ENTRIES:]
    for j in recent:
        mark = "✓" if j.get("status") == "success" else "✗"
        detail = j.get("error") or ""
        if detail:
            detail = f" — {detail[:120]}"
        lines.append(f"- {mark} job {j.get('id')} ({j.get('action')}, {j.get('backend')}) "
                     f"finished {j.get('finished_at')}{detail}")
    if not recent:
        lines.append("_no completions yet_")
    return "\n".join(lines)


def creature_doc(status: dict) -> str:
    """The creature's live snapshot as a readable doc for any bot turn.

    Keys follow the app's RuntimeStatus snapshot (get_status): fps,
    uptime_secs, creature_count, model_count, tool_count, memory_chunks.
    A missing key shows '?' — honest, never a guess."""
    return (
        "# Creature status\n\n"
        f"- fps: {status.get('fps', '?')}\n"
        f"- uptime: {status.get('uptime_secs', '?')}s\n"
        f"- creatures: {status.get('creature_count', '?')}\n"
        f"- models: {status.get('model_count', '?')}\n"
        f"- tools: {status.get('tool_count', '?')}\n"
        f"- memory chunks: {status.get('memory_chunks', '?')}\n"
    )


# ---- the stateful part -----------------------------------------------------

def _state_path() -> str:
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), "fleet_events_state.json")


def load_state() -> dict:
    try:
        with open(_state_path(), "r", encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError):
        return {}


def save_state(state: dict) -> None:
    try:
        with open(_state_path(), "w", encoding="utf-8") as f:
            json.dump(state, f)
    except OSError:
        pass  # a missing watermark only re-announces; never fatal


async def watch(settings: Optional[dict] = None) -> None:
    """The add_task coroutine: poll, announce, react. Runs until unloaded."""
    s = settings or {}
    interval = float(s.get("interval_s") or DEFAULT_INTERVAL_S)
    addr = ipc.addr_from_settings(s)
    react = bool(s.get("reactions", True))
    swarm = list(s.get("swarm") or [])
    addrs = instance_addrs(swarm, addr)
    state = load_state()
    announced = set(state.get("announced", []))
    while True:
        try:
            _poll_once(addr, react, state, announced, addrs)
        except Exception:  # noqa: BLE001 - a watcher that dies stops announcing
            import logging
            logging.getLogger("bot.screenbuddy").exception("fleet watcher tick failed")
        await asyncio.sleep(interval)


def _poll_once(addr: str, react: bool, state: dict, announced: set,
               addrs: Optional[dict] = None) -> None:
    from bot import db
    from bot import shared_context

    addrs = addrs or {}
    conn = db.get_conn()
    jobs = terminal_jobs(conn, state.get("watermark"))
    fresh = [j for j in jobs if j.get("id") not in announced]
    if fresh:
        shared_context.write_doc(BOARD_DOC, board_content(fresh), actor="screenbuddy-extension")
        if react:
            for j in fresh:
                # A swarm member's completion celebrates on ITS creature (Pattern M);
                # everything else reacts on the primary, which is always safe.
                target = addrs.get(j.get("instance_id")) or addr
                if ipc.reachable(target, timeout=2.0):
                    _react(target, j)
        announced.update(j.get("id") for j in fresh)
        # Keep the announced set from growing without bound.
        if len(announced) > 500:
            announced.clear()
            announced.update(j.get("id") for j in fresh)
    if fresh:
        state["watermark"] = fresh[-1].get("finished_at")
        save_state(state)
    # The creature's own doc is refreshed every tick, terminal or not.
    if ipc.reachable(addr, timeout=2.0):
        try:
            resp = ipc.send(addr, {"cmd": "get_status"}, timeout=5.0)
            shared_context.write_doc(
                CREATURE_DOC, creature_doc(resp.get("data") or {}), actor="screenbuddy-extension"
            )
        except (OSError, ConnectionError, ValueError):
            pass
    # Pattern N (the verifiable half): the creature's newest exchange becomes
    # a user-model item for the 'creature' instance — the per-person profile
    # the fleet already keeps. The fine-tune half (unsloth training) has no
    # seam in the current API; it is designed in the master plan, not built.
    _sync_user_model(addr, state, announced)


def _sync_user_model(addr: str, state: dict, announced: set) -> None:
    """Fold the creature's newest exchange into the user model.

    The user model (bot/agent_runtime/user_model.py) is the fleet's
    per-person profile; the creature's conversations are exactly the kind
    of signal it exists for. One item per new exchange, keyed by content
    hash in the state file so a restart re-ingests nothing. The 'creature'
    instance owns the items; user_id is the literal "creature" — the
    profile is the creature's own, not a human's. kind is "preference":
    the model's only kinds are preference/expertise/instruction/goal, and
    a creature's exchanges are preference signals about its user.
    """
    from bot import bot_instances
    from bot.agent_runtime import user_model

    if not ipc.reachable(addr, timeout=2.0):
        return
    instances = bot_instances.list_instances()
    creature = next((i for i in instances if i.get("name") == "creature"), None)
    if creature is None:
        return
    try:
        resp = ipc.send(addr, {"cmd": "get_chat_history", "limit": 100}, timeout=10.0)
    except (OSError, ConnectionError, ValueError):
        return
    messages = resp.get("data", {}).get("messages", [])
    assistant = None
    for m in messages:
        if isinstance(m, (list, tuple)) and len(m) == 2 and str(m[0]).strip().lower() == "assistant":
            assistant = str(m[1]).strip()
    if not assistant:
        return
    digest = hashlib.sha1(assistant.encode("utf-8")).hexdigest()[:12]
    if digest in announced:
        return
    try:
        user_model.add_item(
            creature["id"], "creature", "preference",
            assistant[:2000], confidence=0.5, session="creature", turn=0,
        )
    except Exception:  # noqa: BLE001 - a profile item must never kill the watcher
        return
    announced.add(digest)
    if len(announced) > 500:
        announced.clear()
        announced.add(digest)


def _react(addr: str, job: dict) -> None:
    """Celebrate + speak. Animation and voice only — never a prompt, so the
    reaction cannot re-trigger the event it celebrates."""
    ok = job.get("status") == "success"
    try:
        ipc.send(addr, {"cmd": "set_animation", "id": None,
                        "state": "celebrate" if ok else "idle"}, timeout=5.0)
        ipc.send(addr, {"cmd": "speak", "text":
                        f"Fleet job {job.get('id')} {'finished' if ok else 'failed'}."},
                 timeout=5.0)
    except (OSError, ConnectionError, ValueError):
        pass
