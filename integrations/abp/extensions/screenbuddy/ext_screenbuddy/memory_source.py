"""A memory-fabric source: the creature's own RAG, one direction.

Registered with add_memory_source — zero ABP core edits, the same hook
the built-in "folder"/"web"/"conversation" kinds use. The sync
(memoryfabric/sources.py) calls read(source) with the stored config,
ingests what is new or changed by content hash, removes what vanished,
and snapshots the result — all the fabric's own machinery, unchanged.

One direction by design: creature → fabric is automatic (facts the
creature learned become fleet knowledge); fabric → creature is gated by
the approval flow elsewhere, so the on-device memory stays clean of
unapproved fleet noise.

Source config (backends.screenbuddy or a fabric source entry):
    kind   "screenbuddy" (fixed)
    addr   the creature's IPC address, e.g. 127.0.0.1:34567
    query  what to pull, e.g. "fleet", "project", "*" for everything
    limit  max passages per sync (default 50, the fabric's own budgets cap it)
"""
from __future__ import annotations

from typing import Any, Iterator

from . import ipc

REQUIRED = ["addr"]


def read(source: dict) -> Iterator[tuple]:
    """Yield (item_id, title, text, timestamp, tags) for what the creature
    holds now. A creature that isn't running yields nothing — the next
    sync finds it, which is why the watcher is polled, not pushed."""
    if not ipc.reachable(ipc.addr_from_settings(source), timeout=2.0):
        return
    query = str(source.get("query") or "*")
    limit = int(source.get("limit") or 50)
    try:
        resp = ipc.send(
            ipc.addr_from_settings(source),
            {"cmd": "memory_search", "query": query, "limit": limit},
            timeout=15.0,
        )
    except (OSError, ConnectionError, ValueError):
        return  # transient: the fabric retries on the next sync
    data = resp.get("data") if isinstance(resp, dict) else None
    hits = []
    if isinstance(data, list):
        hits = data
    elif isinstance(data, dict):
        hits = data.get("results") or data.get("passages") or []
    for i, hit in enumerate(hits):
        item_id, title, text, ts, tags = _shape(hit, i, source)
        if text:
            yield (item_id, title, text, ts, tags)


def _shape(hit: Any, index: int, source: dict) -> tuple:
    """Normalize a memory_search hit into the fabric's item tuple. The app's
    passage shape is {source, text, score?}; be tolerant of either dict or
    plain-string hits so a shape change degrades instead of breaking a sync."""
    label = str(source.get("label") or "screenbuddy")
    if isinstance(hit, dict):
        text = str(hit.get("text") or hit.get("content") or "")
        origin = str(hit.get("source") or hit.get("id") or f"passage-{index}")
        score = hit.get("score")
        tags = [label, "creature-memory"]
        if score is not None:
            tags.append(f"score:{score}")
        return (f"{origin}#{index}", f"{label}: {origin}", text, None, tags)
    text = str(hit)
    return (f"passage-{index}", f"{label}: passage-{index}", text, None, [label])
