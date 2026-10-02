# ScreenBuddy Control API

ScreenBuddy exposes a live control surface over TCP so an external agent — Hermes,
via the bundled MCP server — can drive the running desktop app.

```
Hermes  ──MCP (stdio)──▶  mcp/server.py  ──TCP JSON──▶  ScreenBuddy app
             18 tools                        127.0.0.1:34567
```

## Quick start

```bash
# 1. Build the app once (the MCP server will start it on demand)
cargo build --release

# 2. Register the MCP server with Hermes (one time)
hermes mcp add screenbuddy --command python --args "<repo>/mcp/server.py"

# 3. Start a new Hermes session, or run /reload-mcp
```

Tools then appear as `mcp_screenbuddy_*`.

You do **not** need to start the app yourself. The server resolves the executable
relative to its own location (`target/release`, then `target/debug`, then `dist`,
then `PATH`) and launches it if nothing is answering on the control port, so a
tool call works from a cold start.

## Configuration

| Variable | Default | Purpose |
|---|---|---|
| `SCREENBUDDY_IPC_ADDR` | `127.0.0.1:34567` | Control port (app side) |
| `SCREENBUDDY_IPC_TIMEOUT` | `10` | Per-command socket timeout, seconds |
| `SCREENBUDDY_DISABLE_IPC` | unset | Set on the app to disable the control server |
| `SCREENBUDDY_EXE` | auto | Explicit path to the executable to launch |
| `SCREENBUDDY_AUTOSTART` | `1` | Set to `0` to never launch the app |

## Tools

| Tool | Effect |
|---|---|
| `screenbuddy_ping` | Liveness check |
| `screenbuddy_status` | fps, uptime, creatures, models, memory |
| `screenbuddy_list_creatures` | Position/visibility/state per creature |
| `screenbuddy_set_animation` | Force idle/walk/fly/sleep/celebrate |
| `screenbuddy_move_creature` | Move and **pin** a creature |
| `screenbuddy_set_visible` | Show/hide a creature |
| `screenbuddy_set_auto_cycle` | Pause automatic animation rotation |
| `screenbuddy_send_chat` | Message the AI chat |
| `screenbuddy_chat_history` | Read the transcript |
| `screenbuddy_run_agent` | Run the tool-using agent |
| `screenbuddy_agent_info` | Registered tools, last call/response |
| `screenbuddy_play_sound` | Play a sound effect |
| `screenbuddy_speak` | Text-to-speech |
| `screenbuddy_set_setting` / `get_setting` | Read/write runtime settings |
| `screenbuddy_memory_ingest` / `memory_search` | RAG memory |
| `screenbuddy_list_sounds` | Bundled sound names |

### Two behaviours worth knowing

**Pinning.** `move_creature` pins the position. Physics integrates velocity every
frame, so an unpinned move would drift away immediately.

**Auto-cycle.** The app rotates animation states on a timer. Turn it off with
`set_auto_cycle(false)` before forcing a state, or the rotation overwrites it
within five seconds.

## Wire protocol

Length-prefixed JSON: 4-byte big-endian length, then that many bytes of UTF-8
JSON. One request, one response, then the connection may close.

Request:

```json
{"cmd": "set_animation", "id": "companion-bird-01", "state": "sleep"}
```

Response:

```json
{"status": "success", "data": {"queued": true}}
```

Errors are `{"status": "error", "message": "...", "code": 400}`. A control
request is rejected with code `503` when nothing is draining the app's queue, so a
disconnected app is reported rather than silently dropping the request.

Frames above 8 MB are refused.

## Verifying a change

```bash
# Protocol-level checks against a running app
python tools/ipc_probe.py

# MCP-level checks: handshake, tool discovery, and tool calls
python mcp/test_mcp.py
```

`mcp/test_mcp.py` includes a cold-start check: it stops the app, confirms the
port is closed, and verifies the server brings it back up. It reports SKIP when
autostart is disabled or no executable is present.

## Security

The control port binds to `127.0.0.1` and has no authentication, so any local
process can drive the app. That is appropriate for a single-user desktop
companion, but do not expose the port on a shared machine. If you need remote
access, put an authenticated proxy in front of it.