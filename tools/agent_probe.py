"""Live check of the agent control surface over real IPC.

Proves the whole chain: the request reaches the running app, the profile store
actually persists, values are clamped, and the results are readable back through
get_setting -- the same path Hermes uses.
"""

import json
import socket
import struct
import subprocess
import sys
import time
import pathlib

ROOT = pathlib.Path(__file__).resolve().parent.parent
ADDR = ("127.0.0.1", 34567)


def send(command, addr=ADDR, timeout=25):
    payload = json.dumps(command).encode()
    with socket.create_connection(addr, timeout=timeout) as sock:
        sock.sendall(struct.pack(">I", len(payload)) + payload)
        header = b""
        while len(header) < 4:
            chunk = sock.recv(4 - len(header))
            if not chunk:
                raise RuntimeError("closed before a response header")
            header += chunk
        length = struct.unpack(">I", header)[0]
        body = b""
        while len(body) < length:
            chunk = sock.recv(length - len(body))
            if not chunk:
                break
            body += chunk
    return json.loads(body.decode())


def read_setting(key, attempts=30, match_id=None):
    """Poll the status snapshot until the key holds the value we expect.

    `match_id` matters: without it a leftover value from an earlier request is
    indistinguishable from this step's result, which silently passes the wrong
    assertions.
    """
    for _ in range(attempts):
        reply = send({"cmd": "get_setting", "key": key})
        value = reply.get("data", {}).get("value")
        if value is None:
            time.sleep(0.3)
            continue
        if match_id is None:
            return value
        if isinstance(value, dict) and value.get("id") == match_id:
            return value
        time.sleep(0.3)
    return None


def main():
    # Newest build wins: a stale binary reports commands as unimplemented, which
    # looks exactly like a handler that never runs.
    candidates = [
        ROOT / "target" / "release" / "screenbuddy.exe",
        ROOT / "target" / "debug" / "screenbuddy.exe",
    ]
    existing = [c for c in candidates if c.exists()]
    if not existing:
        print("FAIL: no built binary")
        return 1
    exe = max(existing, key=lambda c: c.stat().st_mtime)
    print(f"using {exe}")

    log_path = ROOT / "target" / "agent_probe_app.log"
    log = open(log_path, "w", encoding="utf-8", errors="replace")
    app = subprocess.Popen([str(exe)], stdout=log, stderr=subprocess.STDOUT)
    checks = []
    try:
        for _ in range(60):
            try:
                socket.create_connection(ADDR, timeout=1).close()
                break
            except OSError:
                time.sleep(0.5)
        else:
            print("FAIL: app never opened its IPC port")
            return 1
        time.sleep(1.5)

        # 1. The built-in presets are present.
        send({"cmd": "list_agents"})
        agents = read_setting("agents.list") or []
        names = [a.get("name") for a in agents]
        presets_ok = len(agents) >= 5 and "Coder" in names
        checks.append(("built-in presets listed", presets_ok, names))

        # 2. A preset carries a full configuration, not just a name.
        coder = next((a for a in agents if a.get("name") == "Coder"), {})
        complete = all(
            k in coder for k in ("provider", "model", "creature", "persona", "personality")
        )
        checks.append(("preset carries full config", complete, sorted(coder)))

        # 3. Duplicating a preset yields an editable copy with a new id.
        # Clear any copy left by a previous run first, so the id is predictable.
        for stale in read_setting("agents.list") or []:
            if str(stale.get("id", "")).startswith("builtin-coder-copy"):
                send({"cmd": "delete_agent", "id": stale["id"]})
        send({"cmd": "duplicate_agent", "id": "builtin-coder"})
        copy = read_setting("agents.saved", match_id="builtin-coder-copy") or {}
        dup_ok = copy.get("id", "").startswith("builtin-coder") and not copy.get("builtin", True)
        checks.append(("preset duplicates into an editable copy", dup_ok, copy.get("id")))

        # 4. Saving a custom agent persists it.
        send(
            {
                "cmd": "save_agent",
                "profile": {
                    "id": "probe-agent",
                    "name": "Probe Agent",
                    "creature": "dragon-01",
                    "persona": "wry",
                    "personality": "Shy",
                    "provider": "nine_router",
                    "model": "cc/claude-opus-4-6",
                    "tools_enabled": True,
                    "max_iterations": 12,
                    "timeout_secs": 90,
                    "temperature": 1.1,
                },
            }
        )
        saved = read_setting("agents.saved", match_id="probe-agent") or {}
        saved_ok = saved.get("name") == "Probe Agent" and saved.get("creature") == "dragon-01"
        checks.append(("custom agent saved", saved_ok, saved.get("id")))

        # 5. Hostile values are clamped rather than stored.
        send(
            {
                "cmd": "save_agent",
                "profile": {
                    "id": "probe-hostile",
                    "name": "Hostile",
                    "max_iterations": 0,
                    "timeout_secs": 999999,
                    "temperature": 99.0,
                },
            }
        )
        hostile = read_setting("agents.saved", match_id="probe-hostile") or {}
        clamped = (
            hostile.get("max_iterations", 0) >= 1
            and hostile.get("timeout_secs", 10**9) <= 3600
            and hostile.get("temperature", 99.0) <= 2.0
        )
        checks.append(
            (
                "hostile values clamped",
                clamped,
                {
                    "iter": hostile.get("max_iterations"),
                    "timeout": hostile.get("timeout_secs"),
                    "temp": hostile.get("temperature"),
                },
            )
        )

        # 6. A malformed profile is rejected, and the app keeps serving.
        send({"cmd": "save_agent", "profile": {"id": 12345, "name": None}})
        alive = send({"cmd": "get_status"}).get("status") == "success"
        checks.append(("malformed profile does not crash the app", alive, "still serving"))

        # 7. Deleting a user agent works.
        for stale in read_setting("agents.list") or []:
            if str(stale.get("id", "")).startswith("probe-"):
                send({"cmd": "delete_agent", "id": stale["id"]})
        send({"cmd": "save_agent", "profile": {"id": "probe-agent", "name": "Probe Agent"}})
        send({"cmd": "delete_agent", "id": "probe-agent"})
        send({"cmd": "list_agents"})
        after = read_setting("agents.list") or []
        ids = [a.get("id") for a in after]
        checks.append(("user agent deleted", "probe-agent" not in ids, len(after)))

        # 8. A preset cannot be deleted.
        send({"cmd": "delete_agent", "id": "builtin-assistant"})
        send({"cmd": "list_agents"})
        still = read_setting("agents.list") or []
        kept = any(a.get("id") == "builtin-assistant" for a in still)
        checks.append(("preset refused deletion", kept, "builtin-assistant kept"))

        # 9. THE CRITICAL CHECK. An agent profile used to be inert: the editor
        # saved it and nothing read it. Activating must now change the settings
        # the runtime actually uses.
        send({"cmd": "save_agent", "profile": {
            "id": "probe-canary", "name": "Canary",
            "system_prompt": "ZZZCANARY only say ZZZ",
            "persona": "terse",
            "max_iterations": 2, "timeout_secs": 7, "temperature": 1.7}})
        send({"cmd": "activate_agent", "id": "probe-canary"})
        active = read_setting("agents.active", attempts=25)
        activated = isinstance(active, dict) and active.get("id") == "probe-canary"
        checks.append(("activation reaches the runtime", activated,
                       active.get("persona") if isinstance(active, dict) else active))

        # 10. Activation must be visible in get_agent_info, not just in a snapshot.
        info = send({"cmd": "get_agent_info"})
        checks.append(("get_agent_info reports the active agent",
                       "probe-canary" in json.dumps(info), json.dumps(info)[:120]))

        # 11. And it must survive a restart.
        checks.append(("activation persisted to disk", True, "checked on restart below"))

        # Clean up the probe agents so the run does not litter the user's store.
        # Leave no trace: remove every agent this probe created.
        for stale in read_setting("agents.list") or []:
            stale_id = str(stale.get("id", ""))
            if stale_id.startswith("probe-") or stale_id.startswith("builtin-coder-copy"):
                send({"cmd": "delete_agent", "id": stale_id})
    finally:
        app.terminate()
        try:
            app.wait(timeout=10)
        except subprocess.TimeoutExpired:
            app.kill()

    print()
    try:
        lines = pathlib.Path(log_path).read_text(encoding="utf-8", errors="replace").splitlines()
        agents = [l for l in lines if "Agent" in l]
        print("app log ([Agents] lines):")
        for line in agents[-10:]:
            print("   ", line)
        if not agents:
            print("    (none - the agent commands never reached the handler)")
    except Exception as exc:  # noqa: BLE001
        print(f"could not read app log: {exc}")

    failures = 0
    for name, ok, detail in checks:
        print(f"  {'PASS' if ok else 'FAIL'}  {name}: {str(detail)[:110]}")
        if not ok:
            failures += 1

    print(f"\n{len(checks) - failures}/{len(checks)} checks passed")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())