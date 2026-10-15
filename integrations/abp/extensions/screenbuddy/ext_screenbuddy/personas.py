"""Persona continuity: ABP instances ↔ creature agents.

Neither system has identity across resets alone: ABP's agent_messages
reset on /new, the creature's chat history resets between app sessions.
The map is the bridge — an ABP instance is answered by a named creature
agent, activated before the turn and seeded with the instance's own
memory afterwards.

Config (backends.screenbuddy or the extension's settings):
    personas:
      screenbuddy-test: assistant
      research-bot: researcher
    seed: true          # ingest the exchange into the creature's RAG

The map is deliberately small: a hand-written pairing in config beats an
automatic scheme nobody understands when it goes wrong.
"""
from __future__ import annotations

import json
from typing import Any, Optional

from . import ipc


def parse_personas(settings: Optional[dict]) -> dict[str, str]:
    """The instance→agent map, from a settings dict (or a JSON string for
    env-style configuration). A malformed value yields an empty map: an
    unmapped instance just answers as the creature's default persona."""
    if not settings:
        return {}
    raw = settings.get("personas")
    if isinstance(raw, str):
        try:
            raw = json.loads(raw)
        except ValueError:
            return {}
    if not isinstance(raw, dict):
        return {}
    return {str(k): str(v) for k, v in raw.items() if v}


def persona_for(target_instance: str, settings: Optional[dict]) -> Optional[str]:
    """Which creature agent answers this instance, if any."""
    return parse_personas(settings).get(target_instance)


def activate(persona_id: str, addr: str) -> bool:
    """Switch the creature's active agent before the turn. False when the
    creature is unreachable — the turn still runs, as the default persona."""
    if not ipc.reachable(addr, timeout=2.0):
        return False
    try:
        resp = ipc.send(addr, {"cmd": "activate_agent", "id": persona_id}, timeout=5.0)
    except (OSError, ConnectionError, ValueError):
        return False
    return isinstance(resp, dict) and resp.get("status") == "success"


def seed(summary: str, addr: str, source: str = "abp-turn") -> bool:
    """Ingest an exchange into the creature's RAG, so the next session
    recalls it. Best-effort: an unreachable creature loses one seed."""
    if not summary or not ipc.reachable(addr, timeout=2.0):
        return False
    try:
        resp = ipc.send(
            addr, {"cmd": "memory_ingest", "source": source, "content": summary}, timeout=5.0
        )
    except (OSError, ConnectionError, ValueError):
        return False
    return isinstance(resp, dict) and resp.get("status") == "success"


def turn_summary(prompt: str, reply: str, limit: int = 400) -> str:
    """A compact, seedable record of one turn. Small on purpose — the RAG
    holds passages, and a wall of text seeds nothing recallable."""
    def clip(text: str) -> str:
        text = " ".join(str(text).split())
        return text if len(text) <= limit else text[: limit - 1] + "…"
    return f"Q: {clip(prompt)}\nA: {clip(reply)}"
