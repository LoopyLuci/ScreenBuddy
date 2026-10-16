# ScreenBuddy × AgenticBotPlatform integration

Zero ABP core edits: everything here is an ABP **extension** (`ext_screenbuddy/`,
registered through `api.add_*` hooks) plus a **config overlay** (`config/backends.yaml`,
deep-merged over ABP's `config/backends.yaml` by `bot/config.py`'s overlay chain).

## Activate

1. Start the desktop app (it listens on `127.0.0.1:34567`).
2. Point ABP at this folder and start it:

   ```
   ABP_PRIVATE_DIR=C:/Projects/ScreenBuddy/integrations/abp
   DASHBOARD_TOKEN=<a long random token>
   python -m bot.main
   ```

3. Set a bot instance's backend to `screenbuddy` (dashboard, `/api/bots/{id}/model`, or the
   bots API). The instance's turns are now answered by the creature.

## What the extension wires

| Piece | Hook | What it does |
|---|---|---|
| Backend kind | `add_backend_class` | `backend: screenbuddy` → one `abp_client.py` process per turn, in a `process_cell` |
| Learning loop | `add_routes` | `POST /api/screenbuddy/outcome` (record a model-call outcome), `POST /api/screenbuddy/recommend` (the router's pick), `GET /api/screenbuddy/status` (panel data) |
| Memory | `add_memory_source` | the creature's RAG becomes fleet knowledge (`kind: screenbuddy`, config `addr`/`query`/`limit`) |
| Events | `add_task` | terminal jobs → `fleet-board` shared-context doc + Celebrate on the creature; `creature-status` doc refreshed every tick |
| Personas | backend settings | `personas: {instance: agent_id}` — activate the mapped creature agent before each turn |
| Panel | `add_panel` | the **Creature** dashboard panel — live fps/uptime/creatures/models/tools/memory |
| Scoped key | `add_integration_scope` + `add_integration_preset` | mint a ScreenBuddy key limited to exactly its own routes (see below) |
| User model | watcher task | the creature's newest exchange becomes a pending `preference` item for the `creature` instance |
| Fine-tune | `POST /api/screenbuddy/finetune` | approved user-model items → a real JSONL dataset; a LoRA trains where torch+CUDA+unsloth exist (honest skip otherwise) |
| Swarm | overlay `swarm:` list | N instances on distinct ports, each its own backend kind; a swarm answer celebrates on its own creature |
| Sandbox | `POST /api/screenbuddy/sandbox_exec` | a dangerous tool runs in a bounded process_cell (Job Object) with CPU/memory/time limits; the app routes dangerous verbs through it |

## Security: use a scoped key, not the god-mode token

`agent_control` defaults to `trust_all` (any instance may target any other) — fine for a dev
machine, not for anything exposed. The extension registers an integration-key scope so the
creature can hold a key limited to exactly what it needs:

```
POST /api/integrations/keys   {name: "screenbuddy", preset: "screenbuddy"}
```

The preset grants `POST /api/agent/ask`, `POST /api/screenbuddy/(outcome|recommend)` and
`GET /api/agent/status` — nothing else. A leaked creature key cannot reconfigure the fleet,
read other bots' data, or mint further keys. Point the app's `DASHBOARD_TOKEN` at that key
instead of the dashboard token.

## Config overlay

`config/backends.yaml` (this folder) deep-merges over ABP's tracked file:

```yaml
backends:
  screenbuddy:
    addr: 127.0.0.1:34567     # the creature's IPC address
    timeout_s: 300            # a small local model takes 40-60s; the router default kills it
    personas:                 # Pattern K: instance name -> creature agent id
      creature: builtin-assistant
    seed: true                # ingest each exchange into the creature's RAG
    interval_s: 10            # fleet-event watcher tick
    reactions: true           # Celebrate + speak on job completions
```

## Tests

```
PYTHONPATH=C:/Projects/AgenticBotPlatform python tests/test_ext_screenbuddy.py
```

19 unit tests cover the memory source (live/down/transient), the event diff and board
rendering, persona mapping, and route validation. ABP's pipeline also picks the directory up
via `add_test_dir`.

## The one rule that matters

Never route the app through ABP while an ABP bot is routed through that same app — the two
sides wait on each other and deadlock. One direction per app instance.
