"""Dashboard routes: the learning loop's two missing halves.

    POST /api/screenbuddy/outcome      record one model-call outcome into
                                       the learning router (reliability,
                                       speed, cooldowns, per-class quality)
    POST /api/screenbuddy/recommend    ask the router which model it would
                                       pick for a task, before the creature
                                       spends the turn

ScreenBuddy's gen_abp already carries a correlation id and task class on
every ask; these routes close the loop — the fleet's router learns from
the creature's calls, and the creature can ask the fleet's brain which
model to use. Both follow ABP's own route convention: reads accept the
dashboard token or a paired device's key, writes need the token itself.
"""
from __future__ import annotations

from typing import Any, Optional

from fastapi import Depends


def register(app, read_auth, write_auth) -> None:
    """Called exactly like an ABP route module's register: the FastAPI app
    plus the two auth callables as dependencies."""
    from bot.router_brain import learn
    from bot import model_router

    @app.post("/api/screenbuddy/outcome", dependencies=[Depends(write_auth)])
    async def post_outcome(payload: dict) -> dict:
        """{ref, ok, task_class?, latency_ms?, tokens?, error?, decision_id?}.

        `ref` is "provider/model" as the router writes it (the same string
        a bot turn would record). An unknown ref records nothing — the
        router's own rule, kept here too so a typo can't invent a model.
        """
        problem = validate_outcome(payload)
        if problem:
            return {"ok": False, "error": problem}
        kind = learn.observe(
            str(payload["ref"]),
            ok=bool(payload.get("ok", True)),
            task_class=payload.get("task_class") or None,
            latency_ms=payload.get("latency_ms"),
            tokens=payload.get("tokens"),
            error=str(payload.get("error") or ""),
            decision_id=payload.get("decision_id"),
        )
        return {"ok": True, "failure_kind": kind}

    @app.post("/api/screenbuddy/recommend", dependencies=[Depends(read_auth)])
    async def post_recommend(payload: dict) -> dict:
        """{task, candidates?, context_tokens?} -> the router's pick.

        The creature asks before a local turn: the fleet's learned
        preference (reliability, speed, cooldowns, the naive Bayes task
        classifier) plus the user's 9Router prefer/avoid/block constraints,
        decided in one place instead of three.
        """
        problem = validate_recommend(payload)
        if problem:
            return {"ok": False, "error": problem}
        try:
            rec = model_router.recommend(
                str(payload["task"]),
                candidates=payload.get("candidates"),
                context_tokens=int(payload.get("context_tokens") or 0),
            )
        except Exception as exc:  # noqa: BLE001 - a recommendation that fails is a non-answer
            return {"ok": False, "error": str(exc)[:400]}
        return {"ok": True, "recommendation": rec}


    @app.get("/api/screenbuddy/status", dependencies=[Depends(read_auth)])
    async def get_status(payload: dict = None) -> dict:
        """The creature's live snapshot for the dashboard panel. A creature
        that isn't running is a 503 with the reason — the panel shows
        "not running" rather than a stale number."""
        from . import ipc

        addr = (payload or {}).get("addr") or _panel_addr()
        if not ipc.reachable(addr, timeout=2.0):
            return {"ok": False, "error": f"ScreenBuddy is not running on {addr}"}
        try:
            resp = ipc.send(addr, {"cmd": "get_status"}, timeout=5.0)
        except (OSError, ConnectionError, ValueError) as exc:
            return {"ok": False, "error": str(exc)[:200]}
        return {"ok": True, "data": resp.get("data") or {}}

    # --- Pattern N's fine-tune half: traces -> dataset -> (GPU) a LoRA -----
    @app.post("/api/screenbuddy/finetune", dependencies=[Depends(write_auth)])
    async def post_finetune(payload: dict = None) -> dict:
        """Build the training dataset from the user model's approved items,
        then train when — and only when — this machine can. The response
        always says which branch ran; a skipped training is a skipped
        training, never a silent success."""
        from . import finetune

        ds = finetune.build_dataset(limit=int((payload or {}).get("limit") or 200))
        result = {"ok": ds["ok"], "dataset": ds}
        if ds.get("path") and finetune.backend_status().get("trainable"):
            result["training"] = finetune.train_adapter(ds["path"])
            if result["training"].get("adapter"):
                # The adapter joins Local AI's Discover under this app's
                # namespace, so the router can offer it like any local model.
                api_model_location(
                    "screenbuddy", result["training"]["adapter"], "screenbuddy"
                )
        else:
            result["training"] = {
                "trained": False, "skipped": True,
                "reason": finetune.backend_status().get("reason"),
            }
        return result

    @app.get("/api/screenbuddy/finetune/status", dependencies=[Depends(read_auth)])
    async def get_finetune_status() -> dict:
        """Dataset artifacts, training backend, adapter state."""
        from . import finetune

        return finetune.status()

    # --- Pattern L: dangerous tools in a bounded cell ----------------------
    @app.post("/api/screenbuddy/sandbox_exec", dependencies=[Depends(write_auth)])
    async def post_sandbox_exec(payload: dict) -> dict:
        """Run one tool inside a process_cell and return its bounded output.

        The creature's agent routes anything that shells out, writes files
        or changes state through here (see mcp_client.rs's dangerous list):
        the tool executes in a Windows Job Object with CPU, memory and
        wall-clock limits, so a runaway cannot take the dashboard's process
        with it. A timeout kills the whole tree and says so.
        """
        from . import sandbox_exec

        tool = str(payload.get("tool") or "")
        if not tool:
            return {"ok": False, "error": "tool is required"}
        return sandbox_exec.execute(
            tool,
            payload.get("args") or {},
            timeout_s=payload.get("timeout_s") or 60,
        )


def api_model_location(app_name: str, path: str, namespace: str) -> None:
    """Register a trained adapter with Local AI's Discover, best-effort.

    The registry is the extension api's; reaching it from a route keeps the
    registration call next to its only caller, and a registry that isn't
    there (older core) simply means the adapter is not discovered — the
    file on disk is real either way.
    """
    try:
        from bot import extensions as _ext

        for ext in _ext.loaded().extensions:
            if ext.name == "screenbuddy" and getattr(ext.api, "model_locations", None) is not None:
                ext.api.add_model_location(app_name, path, namespace)
                return
    except Exception:  # noqa: BLE001 - discovery is an enhancement, never a requirement
        pass


def _panel_addr() -> str:
    import os

    return os.environ.get("SCREENBUDDY_IPC_ADDR") or "127.0.0.1:34567"


def validate_outcome(payload: dict) -> Optional[str]:
    """The outcome payload's own rule, kept next to the router's: a ref
    without a provider/model slash records nothing."""
    ref = str(payload.get("ref") or "")
    if not ref or "/" not in ref:
        return "ref must be provider/model"
    return None


def validate_recommend(payload: dict) -> Optional[str]:
    if not str(payload.get("task") or ""):
        return "task is required"
    return None
