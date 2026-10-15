"""Unit tests for the screenbuddy extension's pure logic.

Run by ABP's pipeline (add_test_dir) or directly:
    python -m pytest integrations/abp/extensions/screenbuddy/tests/
"""
from __future__ import annotations

import sqlite3
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from ext_screenbuddy import fleet_events, memory_source, personas, routes  # noqa: E402


# ---- memory source --------------------------------------------------------

def test_read_yields_items_from_the_creature(monkeypatch):
    monkeypatch.setattr(memory_source.ipc, "reachable", lambda addr, timeout=2.0: True)
    monkeypatch.setattr(
        memory_source.ipc,
        "send",
        lambda addr, cmd, timeout=10.0: {
            "status": "success",
            "data": [
                {"source": "doc-1", "text": "the fleet uses 9Router", "score": 0.9},
                {"source": "doc-2", "text": "the creature answers in 5.4s", "score": 0.8},
            ],
        },
    )
    items = list(memory_source.read({"addr": "127.0.0.1:34567", "query": "fleet"}))
    assert len(items) == 2
    item_id, title, text, ts, tags = items[0]
    assert item_id == "doc-1#0"
    assert "9Router" in text
    assert "creature-memory" in tags


def test_read_yields_nothing_when_the_creature_is_down(monkeypatch):
    monkeypatch.setattr(memory_source.ipc, "reachable", lambda addr, timeout=2.0: False)
    assert list(memory_source.read({"addr": "127.0.0.1:34567"})) == []


def test_read_survives_a_transient_ipc_error(monkeypatch):
    monkeypatch.setattr(memory_source.ipc, "reachable", lambda addr, timeout=2.0: True)

    def boom(addr, cmd, timeout=10.0):
        raise ConnectionError("creature restarted")

    monkeypatch.setattr(memory_source.ipc, "send", boom)
    assert list(memory_source.read({"addr": "127.0.0.1:34567"})) == []


def test_shape_tolerates_plain_string_hits():
    item_id, title, text, ts, tags = memory_source._shape("just text", 3, {})
    assert text == "just text"
    assert item_id == "passage-3"


# ---- fleet events ----------------------------------------------------------

def _jobs_conn():
    conn = sqlite3.connect(":memory:")
    conn.execute(
        "CREATE TABLE jobs (id INTEGER PRIMARY KEY, action_type TEXT, backend TEXT, "
        "status TEXT, error TEXT, prompt TEXT, result TEXT, tokens INTEGER, "
        "created_at TEXT, started_at TEXT, finished_at TEXT, duration_ms INTEGER, "
        "instance_id INTEGER, swarm_run_id TEXT)"
    )
    return conn


def test_terminal_jobs_only_returns_finished_rows_after_the_watermark():
    conn = _jobs_conn()
    rows = [
        (1, "chat", "ollama", "success", None, "2026-10-15T10:00:00"),
        (2, "chat", "ollama", "failed", "boom", "2026-10-15T10:01:00"),
        (3, "chat", "ollama", "running", None, None),
        (4, "chat", "ollama", "success", None, "2026-10-15T10:05:00"),
    ]
    conn.executemany(
        "INSERT INTO jobs (id, action_type, backend, status, error, finished_at) "
        "VALUES (?, ?, ?, ?, ?, ?)",
        rows,
    )
    jobs = fleet_events.terminal_jobs(conn, "2026-10-15T10:00:30")
    assert [j["id"] for j in jobs] == [2, 4], "running is not news; the watermark holds"


def test_terminal_jobs_without_a_watermark_takes_the_latest():
    conn = _jobs_conn()
    conn.executemany(
        "INSERT INTO jobs (id, action_type, backend, status, error, finished_at) "
        "VALUES (?, ?, ?, ?, ?, ?)",
        [(1, "chat", "ollama", "success", None, "2026-10-15T10:00:00")],
    )
    jobs = fleet_events.terminal_jobs(conn, None)
    assert [j["id"] for j in jobs] == [1]


def test_board_content_marks_success_and_failure():
    jobs = [
        {"id": 1, "action": "chat", "backend": "ollama", "status": "success",
         "error": None, "finished_at": "10:00"},
        {"id": 2, "action": "chat", "backend": "ollama", "status": "failed",
         "error": "boom", "finished_at": "10:01"},
    ]
    board = fleet_events.board_content(jobs)
    assert "✓ job 1" in board
    assert "✗ job 2" in board
    assert "boom" in board


def test_board_content_handles_an_empty_fleet():
    assert "no completions yet" in fleet_events.board_content([])


def test_creature_doc_reports_the_snapshot():
    doc = fleet_events.creature_doc({"fps": 60, "uptime_secs": 3600, "creature_count": 1,
                                     "model_count": 3, "tool_count": 74, "memory_chunks": 12})
    assert "fps: 60" in doc
    assert "uptime: 3600s" in doc
    assert "creatures: 1" in doc
    assert "models: 3" in doc
    assert "tools: 74" in doc
    assert "memory chunks: 12" in doc


def test_creature_doc_marks_missing_keys_honestly():
    doc = fleet_events.creature_doc({})
    assert "fps: ?" in doc
    assert "uptime: ?s" in doc


# ---- personas ---------------------------------------------------------------

def test_parse_personas_reads_a_dict():
    assert personas.parse_personas({"personas": {"a": "assistant"}}) == {"a": "assistant"}


def test_parse_personas_reads_a_json_string():
    assert personas.parse_personas({"personas": '{"a": "assistant"}'}) == {"a": "assistant"}


def test_parse_personas_ignores_garbage():
    assert personas.parse_personas({"personas": "not json"}) == {}
    assert personas.parse_personas({"personas": 42}) == {}
    assert personas.parse_personas(None) == {}


def test_persona_for_returns_the_mapping_or_none():
    settings = {"personas": {"research-bot": "researcher"}}
    assert personas.persona_for("research-bot", settings) == "researcher"
    assert personas.persona_for("other", settings) is None


def test_turn_summary_clips_long_text():
    summary = personas.turn_summary("q" * 1000, "a" * 1000, limit=100)
    assert len(summary) <= 220
    assert summary.startswith("Q: ")


# ---- routes -----------------------------------------------------------------

def test_outcome_validation_requires_a_provider_slash():
    assert routes.validate_outcome({"ref": "ollama/qwen2.5:0.5b"}) is None
    assert routes.validate_outcome({"ref": "qwen2.5:0.5b"}) is not None
    assert routes.validate_outcome({}) is not None


def test_recommend_validation_requires_a_task():
    assert routes.validate_recommend({"task": "write a haiku"}) is None
    assert routes.validate_recommend({}) is not None


# ---- the swarm (Pattern M) -------------------------------------------------

_OVERLAY_DIR = Path(__file__).resolve().parents[1]


def _overlay_api():
    """A mock api whose directory resolves to the real overlay layout."""
    class Api:
        directory = _OVERLAY_DIR
        backends = {}

        def add_backend_class(self, kind, factory, **kw):
            pass

        def add_backend(self, name, **settings):
            self.backends[name] = settings

    return Api()


def _write_overlay(tmp_path, body: str):
    overlay = Path(tmp_path) / "config" / "backends.yaml"
    overlay.parent.mkdir(parents=True, exist_ok=True)
    overlay.write_text(body, encoding="utf-8")
    return overlay


def _tmp_overlay(body: str, monkeypatch):
    """A temp ABP_PRIVATE_DIR holding an overlay, cleaned up by the caller."""
    import shutil
    import tempfile

    root = tempfile.mkdtemp(prefix="sb_overlay_")
    _write_overlay(root, body)
    monkeypatch.setenv("ABP_PRIVATE_DIR", root)
    return root


def test_overlay_settings_reads_the_swarm(monkeypatch):
    from ext_screenbuddy import _overlay_settings

    _tmp_overlay(
        "backends:\n  screenbuddy:\n    addr: 127.0.0.1:34567\n"
        "    swarm:\n      - {name: screenbuddy-2, addr: 127.0.0.1:34568}\n",
        monkeypatch,
    )
    s = _overlay_settings(object())
    assert s["swarm"] == [{"name": "screenbuddy-2", "addr": "127.0.0.1:34568"}]
    assert s["addr"] == "127.0.0.1:34567"


def test_overlay_settings_without_a_private_dir_falls_back_to_the_layout():
    from ext_screenbuddy import _overlay_settings

    # The real overlay sits at <ext dir>/../../config/backends.yaml.
    s = _overlay_settings(_overlay_api())
    assert isinstance(s, dict)


def test_instance_addrs_maps_backend_to_creature_addr(monkeypatch):
    import bot.bot_instances as bi

    monkeypatch.setattr(bi, "list_instances", lambda: [
        {"id": 1, "backend": "screenbuddy"},
        {"id": 2, "backend": "screenbuddy-2"},
        {"id": 3, "backend": "ollama"},
    ])
    swarm = [{"name": "screenbuddy-2", "addr": "127.0.0.1:34568"}]
    addrs = fleet_events.instance_addrs(swarm, "127.0.0.1:34567")
    assert addrs == {1: "127.0.0.1:34567", 2: "127.0.0.1:34568"}


def test_instance_addrs_survives_an_inventory_failure(monkeypatch):
    import bot.bot_instances as bi

    def boom():
        raise RuntimeError("db gone")
    monkeypatch.setattr(bi, "list_instances", boom)
    assert fleet_events.instance_addrs([], "127.0.0.1:34567") == {}


def test_register_swarm_registers_one_backend_per_member(monkeypatch):
    from ext_screenbuddy import _register_swarm

    _tmp_overlay(
        "backends:\n  screenbuddy:\n    swarm:\n"
        "      - {name: screenbuddy-2, addr: 127.0.0.1:34568}\n"
        "      - {name: screenbuddy-3, addr: 127.0.0.1:34569, seed: true}\n",
        monkeypatch,
    )

    class Api:
        def __init__(self):
            self.classes = {}
            self.backends = {}

        def add_backend_class(self, kind, factory, **kw):
            self.classes[kind] = (factory, kw)

        def add_backend(self, name, **settings):
            self.backends[name] = settings

    api = Api()
    _register_swarm(api, lambda s, m=None: object(), lambda: (True, ""))
    assert set(api.classes) == {"screenbuddy-2", "screenbuddy-3"}
    assert api.backends["screenbuddy-2"]["addr"] == "127.0.0.1:34568"
    assert api.backends["screenbuddy-2"]["timeout_s"] == 300
    assert api.backends["screenbuddy-3"]["seed"] is True


def test_register_swarm_skips_members_without_an_addr(monkeypatch):
    from ext_screenbuddy import _register_swarm

    _tmp_overlay(
        "backends:\n  screenbuddy:\n    swarm:\n"
        "      - {name: screenbuddy-2}\n"
        "      - {addr: 127.0.0.1:34569}\n",
        monkeypatch,
    )

    class Api:
        def __init__(self):
            self.classes = {}
            self.backends = {}

        def add_backend_class(self, kind, factory, **kw):
            self.classes[kind] = factory

        def add_backend(self, name, **settings):
            self.backends[name] = settings

    api = Api()
    _register_swarm(api, lambda s, m=None: object(), lambda: (True, ""))
    assert set(api.backends) == {"screenbuddy-2"}


def test_watcher_settings_pass_the_swarm_through(monkeypatch):
    from ext_screenbuddy import _watcher_settings

    _tmp_overlay(
        "backends:\n  screenbuddy:\n    addr: 127.0.0.1:34567\n"
        "    swarm:\n      - {name: screenbuddy-2}\n",
        monkeypatch,
    )

    class Api:
        pass

    s = _watcher_settings(Api())
    assert s["swarm"] == [{"name": "screenbuddy-2"}]
    assert s["addr"] == "127.0.0.1:34567"


# ---- standalone runner ------------------------------------------------------
# ABP's pipeline runs these via its own runner; this main() makes the file
# self-contained too — pytest-style functions, a tiny monkeypatch shim.

class _MonkeyPatch:
    """The methods the tests use: setattr, setenv and undo."""

    def __init__(self):
        self._undo = []

    def setattr(self, target, name, value):
        self._undo.append((target, name, getattr(target, name, None)))
        setattr(target, name, value)

    def setenv(self, name, value):
        import os
        self._undo.append(("env", name, os.environ.get(name)))
        os.environ[name] = value

    def undo(self):
        import os
        for target, name, value in reversed(self._undo):
            if target == "env":
                if value is None:
                    os.environ.pop(name, None)
                else:
                    os.environ[name] = value
            else:
                setattr(target, name, value)
        self._undo = []


def _run_all() -> int:
    import inspect

    mod = sys.modules[__name__]
    failures = 0
    for name, fn in sorted(vars(mod).items()):
        if not (name.startswith("test_") and callable(fn)):
            continue
        mp = _MonkeyPatch()
        kwargs = {}
        if "monkeypatch" in inspect.signature(fn).parameters:
            kwargs["monkeypatch"] = mp
        try:
            fn(**kwargs)
            print(f"PASS {name}")
        except Exception as exc:  # noqa: BLE001 - a runner must report, not raise
            failures += 1
            print(f"FAIL {name}: {exc}")
        finally:
            mp.undo()
    print(f"{'FAILED' if failures else 'ALL PASSED'}: {failures} failure(s)")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(_run_all())
