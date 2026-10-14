# Integrating AgenticBotPlatform into ScreenBuddy — Deep Analysis & Recommendations

*Research date: 2026-10-14. ScreenBuddy @ `e719854` (v0.6.3, CI green, 9/9 jobs). AgenticBotPlatform @ `4e7ae2c` (merge of `feat/lf2-small-model`, same day — its learning-router subsystem examined in full below). Every claim below was verified by reading both codebases, not inferred from docs alone.*

---

## 0. How to read this document

Sections 1–3 establish the two architectures with exact seams (ports, wire formats, function signatures). Section 4 is the ranked integration patterns — **each with concrete code or config you can paste and run**. Sections 5–8 are the security posture, the gap analysis, the failure-mode contract, and the phased build order. If you read only one thing: §4.1 and §4.2 are configuration-only and can be live today; §4.3 is the strategic build.

---

## 1. Executive summary

The projects are **strongly complementary, not competing**:

| | ScreenBuddy | AgenticBotPlatform (ABP) |
|---|---|---|
| Language | Rust (2 crates, 391 lib + 181 bin tests) | Python (525 files in `bot/`) + Tauri shell |
| Shape | Single-user desktop companion | Multi-bot server platform |
| UI | Hand-rolled Win32/GDI creatures, animations, sounds | FastAPI dashboard (`:8080`) + Tauri + Android |
| Channels | Desktop overlay, Android broadcast (27 cmds) | Telegram, Discord, Slack, Matrix, WhatsApp |
| AI engine | `ai.rs` — 6 backends, tier selection | 10 backends + learning router + swarms |
| Agent loop | Minimal (echo, time) | ~57 modules: shell, git, sandbox, taint, subagents, vision |
| MCP | **Server only** (25 tools, stdio) | **Server** (dashboard API) **and client** (stdio/HTTP, OAuth, sampling) |
| Routing | 9Router auto-fallback (`:20128/v1`) | Learned router (`router_brain/`): SQLite decisions, reliability/speed/cooldown learning, versioned policy, pin/prefer/avoid/block rules |
| Small models | — | LF2 small-model mode: trimmed tools + prompt for ≤32k windows |
| Multi-tenancy | None | Bot instances, swarms (5 strategies), cluster |

**Thesis:** ScreenBuddy is the *face* (personality, presence, intimacy, one user); ABP is the *brain and backbone* (multi-channel, learned routing, fleet management). Optimal integration = ScreenBuddy becomes a **presence layer riding ABP's routing and channel fabric** — keeping its identity while gaining ABP's model reach — while ABP gains an embodied, persistent agent surface.

**Build order:** (1) ABP `custom_model` → ScreenBuddy 9Router, config-only; (2) ABP MCP client → ScreenBuddy stdio MCP, config-only; (3) ScreenBuddy `Backend::Abp` → ABP `/api/agent/ask`, ~1 week; (4) ScreenBuddy as an ABP backend kind via extension, ~2 weeks, optional; (5) ScreenBuddy Rust MCP client consuming ABP, ~2–3 weeks, optional.

---

## 2. ScreenBuddy — verified integration surface

### 2.1 AI engine (`crates/screenbuddy-core/src/ai.rs`)

```rust
pub enum Backend { Ollama, LlamaCpp, OpenAi, Anthropic, NineRouter, Custom }
```

- `AiConfig`: per-backend endpoints/keys; defaults `ollama_endpoint=http://localhost:11434`, `default_tier=LocalTiny`, `offline_only=true`.
- `Custom` backend (`gen_custom`, line 1175): OpenAI chat-completions wire format via `custom_endpoint` — the landing point for any OpenAI-compatible provider.
- Model resolution: catalogue + live discovery; `resolve_local_model` (hardened `6198ac7`/`c2aa0c9`) verifies installation before requesting and falls back to an installed model — this matters for the 9Router seam (§4.2).
- `AgentRuntime` (`agent.rs`): tool loop, `AgentProfile` (persona, model pin, temperature, iteration budget), broadcast `AgentEvent`. `register_tool(name, tool)` is the extension point.
- HTTP: one shared `reqwest::Client` (line 209); requests built as `serde_json::json!` bodies; failures return `Error::Network` with the status and body — never a canned reply.

### 2.2 9Router (`nine_router.rs`)

- Default `http://localhost:20128/v1`, optional bearer token (`nine_router_api_key`).
- `Backend::NineRouter` + MCP/Android commands `nine_router_status/start/stop/models/test`.
- **ScreenBuddy's strongest give-to-ABP asset**: a local OpenAI-compatible endpoint with automatic provider fallback, already running.

### 2.3 IPC server (`ipc.rs`)

- Length-prefixed JSON (`>I` u32 header + payload) on `127.0.0.1:34567` (override `SCREENBUDDY_IPC_ADDR`).
- **Optional auth token**: when set, first message must be `{"auth":"<token>"}`. Localhost-only bind.
- Commands include `get_status`, `send_chat`, `get_chat_history`, creature control (`list_creatures`, `set_animation`, `move_creature`, `set_visible`, `set_auto_cycle`, `select_creature`), settings, agents (`list_agents`, `activate_agent`, `save_agent`, `duplicate_agent`, `delete_agent`, `run_agent`, `agent_info`), memory (`memory_ingest`, `memory_search`), audio (`play_sound`, `speak`), and `nine_router_*`.

### 2.4 MCP server (`mcp/server.py`, stdio)

25 tools, thin IPC wrappers; auto-launches the app if down (`SCREENBUDDY_AUTOSTART=0` disables). This is the seam ABP's MCP client consumes directly (§4.1).

### 2.5 Android (`ControlCommands.SUPPORTED`)

27 broadcast commands mirroring the IPC surface.

---

## 3. AgenticBotPlatform — verified integration surface (incl. the new router brain)

### 3.1 Backends (`bot/backends/`)

| Backend | What it is |
|---|---|
| `api` | Anthropic API, single-turn |
| `cli` | Claude Code CLI headless |
| `ui` | pywinauto automation of Claude Desktop |
| `hermes_cli` | Hermes Agent one-shot CLI (`hermes -z`) |
| `hermes_gateway` | spawns `hermes serve --isolated` (port 8799), WebSocket JSON-RPC, session-linked |
| `custom_model` | **any OpenAI-compatible endpoint** via `config/providers.yaml` — runs ABP's *own* tool loop against it |
| `opencode` / `openclaw` | delegate to external agent CLIs (the pattern ScreenBuddy would follow as a backend) |
| `native_agent` | ABP's own tool-use loop |

`custom_model` is the key ABP-side seam: `providers.yaml` entries are `{base_url, protocol: openai}` — ScreenBuddy's 9Router (`http://127.0.0.1:20128/v1`) drops in verbatim. Its transport (`bot/agent_runtime/transports/openai_compatible.py`) posts standard OpenAI chat-completions to `{base_url}/chat/completions`, supports vision via `image_url` blocks, and carries a stable per-conversation `x-opencode-session` header.

### 3.2 The router (`bot/router.py`) — resolution order and backup chain

`Router.ask(prompt, *, action_type="quick_question", user_id=0, backend_override=None, context=None, instance_id=None, swarm_run_id=None, chat_id=None, thread_id=None) -> BackendResult` is the single entry point every backend flows through (it also runs the global e-stop check there — deliberately centralized). Chain resolution order:

1. `backend_override` argument
2. per-instance `action_overrides[action_type]`
3. per-instance default
4. global `config.action_overrides[action_type]`
5. global `config.default_backend`
6. `_with_global_backup` appends `config.backend_backup` (e.g. `opencode, hermes_cli`) to every chain, skipping duplicates — so **every instance gets backend-level fallback by default**
7. on failure, retry once against the resolved entry's backup chain

Backend instances are cached under `name::model_override::hermes_home` and properly shut down on eviction/reload (the leak fixes in `_invalidate`).

### 3.3 The router brain (`bot/router_brain/`) — new in `4e7ae2c`, integration-relevant

- `store.py` — SQLite `data/agent/router.db`: every decision with full reasoning, what happened next, per-model stats, cooldowns, learning log, training examples, policy versions.
- `policy.py` — the router's judgement as an editable, versioned document: score **weights per task class** (components like reliability/speed/cost), classification **keywords**, and **rules** (`pin` / `prefer` / `avoid` / `block`, per task class, optionally until a date). Validated strictly; defaults include `feedback_weight: 2.0`, `similar_example_boost: 0.15`, `block_after: 3`.
- `learn.py` — learns from outcomes (decayed reliability, latency moving average, cooldowns), from feedback (quality), from training examples. **Error classification is precise: 429 → "not now" (short cooldown doubling per repeat); 403/404 → "not for us" (long cooldown).**
- Nothing in the brain ever raises into a user's turn — a locked router.db costs learning, never the reply.
- Dashboard surface (`bot/dashboard/router_api.py`): `/api/router/overview`, `/models`, `/decisions[/{id}]`, `POST /decisions/{id}/feedback`, `POST /simulate`, `/events`, `/policy` (GET/PUT + `/history` + `/rollback`), `/examples` (CRUD), `/models/rest|release|forget`, `/reset`.

**Why this matters for integration:** the router brain is exactly the machinery ScreenBuddy lacks (its tier selection is static). Routing ScreenBuddy's traffic *through* ABP means the creature's requests inherit learned model selection and cooldown-aware fallback for free.

### 3.4 Small-model mode (LF2) — new in `4e7ae2c`

`bot/agent_runtime/small_model.py`: when enabled (`auto` = on for local engines with ≤32k context), a leaner turn is sent — core tool set (read/grep/glob/edit/write + `run_shell`), shorter system prompt, tighter context management. Nothing is removed from ABP; other tools still exist and run if an agent definition names them. Cloud models never affected. **Relevant if ScreenBuddy-as-backend (§4.4) drives small local models.**

### 3.5 Agent runtime (`bot/agent_runtime/`, ~57 modules)

`shell.py`, `persistent_shell.py`, `sandbox.py` + `sandbox_ns` (Win32 Job Objects), `taint.py`, `worktrees.py`, `code_intel.py`, `subagents.py`, `checkpoints.py`, `browser.py`, `vision.py`, `secrets_guard.py`, `net_allowlist.py`, `usage_limits.py`, `estop.py`, `mcp_client.py`, `mcp_pins.py`. This is the real differentiator vs ScreenBuddy's 2-tool loop.

### 3.6 MCP — both directions

- **ABP as MCP server** (`bot/mcp_server.py`, 1506 lines): stdio server over the dashboard API — `get_status`, `list_jobs`, `set_backend`, `create_snapshot`, `ask_instance`, `create_bot_instance`, `run_swarm`, `read_project_context`, skills, etc. Auth via `_token()`.
- **ABP as MCP client** (`bot/agent_runtime/mcp_client.py`): connects ABP's native agent loop to third-party MCP servers — stdio subprocess or remote Streamable HTTP, one long-lived connection per enabled row, optional sampling + OAuth 2.1. **Operator-configured only** (never agent-creatable) — matches ScreenBuddy's trust posture exactly.

### 3.7 Agent-to-agent + dashboard chat routes (ScreenBuddy's exact call targets)

- `POST /api/agent/ask` (`bot/dashboard/routes/agent_control.py`): `{source_instance, target_instance, prompt}` → `{"ok": true, "result": "<text>"}`. Allowlist-gated (`trust_all` vs per-instance), audit-logged, 90s timeout in the MCP wrapper. Source is self-declared (drives allowlist + audit, not a security boundary).
- `POST /api/chat/send-to-bot` (`routes/chat.py`): the full CmdContext/dispatch_command/router.ask pipeline — slash commands work, replies stream to authenticated `/api/ws` sockets.
- `POST /api/chat/send` via `bot/outbox.py`: pushes a message *out* through a live platform SDK (appears to a real Telegram user as if from the bot). `outbox.send_message` is sandbox-guarded (`_refuse_in_sandbox`).

### 3.8 Extensibility

- **Plugin API**: single local `plugin.py`, `setup(api)` → `register_tool`/`register_command`; `dangerous=True` routes through the human-approval gate. Trusted local code (ADR-0007).
- **Extensions API** (`bot/extensions.py`): `add_backend_class(kind, factory, *, family, label)` registers whole backend kinds; hot-reloadable via `POST /api/runtime/units/ext:<name>/swap`.

### 3.9 Dashboard API

FastAPI on `127.0.0.1:8080`, `X-Dashboard-Token` (or API key) auth, per-instance `allowed_user_ids`, confirm-gated destructive actions.

---

## 4. Integration patterns, ranked — with concrete implementation

### 4.1 Pattern A — ABP's agent loop gains ScreenBuddy's 25 tools via MCP (config-only)

**Mechanism.** ABP's `mcp_client.py` already supports stdio subprocess MCP servers, operator-configured. ScreenBuddy's `mcp/server.py` is a stdio MCP server with 25 tools. Register it:

```
ABP Dashboard → Bots tab → external MCP server
  name:       screenbuddy
  transport:  stdio
  command:    python
  args:       ["C:/Projects/ScreenBuddy/mcp/server.py"]
  env:        {"SCREENBUDDY_IPC_ADDR": "127.0.0.1:34567"}
```

(or the `external_mcp_servers` DB row / `/mcp_external` Telegram command — both write the same table.)

**Result.** Every ABP agent turn on a tool-loop backend (`api`, `custom_model`, `native_agent`) can call `screenbuddy_send_chat`, `screenbuddy_set_animation`, `screenbuddy_move_creature`, `screenbuddy_speak`, `screenbuddy_run_agent`, `screenbuddy_memory_search`, `screenbuddy_set_setting`, `screenbuddy_play_sound`, … exactly like built-in tools. A Telegram bot animates the desktop creature and speaks through it.

**Effort:** zero code. **Risk:** low. **Caveat:** ABP's own permission/taint system does not apply *inside* ScreenBuddy's tools — same documented trade as `opencode`/`openclaw`. If exposed to an untrusted channel, gate ScreenBuddy-side (it has per-profile `tools_enabled`).

**Verification:** after registering, `list_mcp_servers` should show `screenbuddy` connected; a prompt like "make the creature wave and say hello" should produce animation + speech on the desktop.

### 4.2 Pattern B — ScreenBuddy's 9Router as an ABP model provider (config-only)

**Mechanism.** `config/providers.yaml`:

```yaml
providers:
  screenbuddy_9router:
    base_url: "http://127.0.0.1:20128/v1"
    protocol: openai
    # api_key_env: NINE_ROUTER_KEY   # only if 9Router runs with auth
```

Set a bot instance's backend to `custom_model`, model `screenbuddy_9router/<model_id>`. ABP's own tool loop runs against it — the model gets ABP's full agentic capability (shell/file/git/sandbox/taint), not passthrough chat. `/gateway` and the dashboard picker list 9Router's live models from `/v1/models`.

**Result.** ABP inherits ScreenBuddy's automatic provider-fallback; 9Router gains a real consumer. Wire format is already verified compatible: ABP's `OpenAICompatibleTransport` posts standard chat-completions; 9Router serves the OpenAI-compatible `/v1` surface that ScreenBuddy's own `Backend::NineRouter` already consumes (and whose install-verification was just hardened — a 404 from a missing model now resolves to an installed one rather than failing).

**Effort:** zero code. **Risk:** none (localhost; optional bearer).

**Synergy worth noting:** ABP's router brain will *learn* about models reached through 9Router (reliability, cooldowns on 429/404) — the two routers stack: 9Router provides provider-level fallback inside one endpoint; ABP's brain provides model-level selection across endpoints. ScreenBuddy keeps 9Router as its own static fallback; nothing is lost.

### 4.3 Pattern C — ScreenBuddy routes through ABP (`Backend::Abp`, ~1 week)

**Mechanism.** ABP exposes no OpenAI-compatible chat *server* (it is a client of such servers) — so ScreenBuddy→ABP goes through the dashboard REST API. The verified call target is `POST /api/agent/ask`:

- Request: `{"source_instance": "<screenbuddy>", "target_instance": "<bot>", "prompt": "<prompt>"}`
- Response: `{"ok": true, "result": "<text>"}`
- Auth: `X-Dashboard-Token: <token>`; allowlist-gated; audit-logged; ~90s timeout.

Sketch (mirrors `gen_openai`'s shape; the `reqwest` client already exists on `AiEngine`):

```rust
// ai.rs — new variant
pub enum Backend { /* ... */ Abp }

// AiConfig additions
pub abp_endpoint: Option<String>,        // e.g. "http://127.0.0.1:8080"
pub abp_api_key: Option<String>,         // dashboard token
pub abp_source_instance: Option<String>,
pub abp_target_instance: Option<String>,

async fn gen_abp(&self, req: &AiRequest, _model: &ModelInfo) -> Result<AiResponse> {
    let ep = self.config.abp_endpoint.as_ref()
        .ok_or_else(|| crate::error::Error::InvalidConfig("No ABP endpoint".into()))?;
    let token = self.config.abp_api_key.as_ref()
        .ok_or_else(|| crate::error::Error::InvalidConfig("No ABP token".into()))?;
    // Compose the prompt from system + history + user input, matching build_messages' semantics.
    let prompt = self.compose_flat_prompt(req);
    let body = serde_json::json!({
        "source_instance": self.config.abp_source_instance
            .clone().unwrap_or_else(|| "screenbuddy".into()),
        "target_instance": self.config.abp_target_instance.clone()
            .ok_or_else(|| crate::error::Error::InvalidConfig("No ABP target instance".into()))?,
        "prompt": prompt,
    });
    let resp = self.client
        .post(format!("{}/api/agent/ask", ep.trim_end_matches('/')))
        .header("X-Dashboard-Token", token)
        .json(&body)
        .send()
        .await
        .map_err(|e| crate::error::Error::Network(e.to_string()))?;
    if !resp.status().is_success() {
        let detail = resp.text().await.unwrap_or_default();
        return Err(crate::error::Error::Network(format!("ABP error: {detail}")));
    }
    let json: serde_json::Value = resp.json().await
        .map_err(|e| crate::error::Error::Network(e.to_string()))?;
    let text = json["result"].as_str().unwrap_or("").to_string();
    Ok(AiResponse::plain(text, ModelTier::Cloud, Backend::Abp, 0, FinishReason::Stop))
}
```

Wire the new arm into `generate()`'s dispatch (line ~391) and `generate_stream()`'s, add the config fields to `AiConfig::default()` (from env, like the OpenAI/Anthropic keys), and register an `Abp` catalogue model so `select_model`/`find_model_by_name` resolve it.

**Critical design rule — the graceful-degradation contract:** when ABP is unreachable (connection refused, 5xx, token rejected), `gen_abp` must return `Error::Network` — and the caller (`ai_bridge.rs`'s `send_message`) already reports the failure rather than inventing a reply (that canned-fallback removal is a documented codebase invariant). Whether to additionally fall back to the existing tier selection is a policy choice for the agent profile, not something to hardcode.

**Result.** ScreenBuddy's `send_chat` answers *as an ABP bot instance*: the router brain picks the model per task class, the backup chain absorbs failures, swarms are available per-turn — while the creature's persona (`AgentProfile.system_prompt`), memory, animations and sounds stay ScreenBuddy-side.

**Tests (TDD, matching the repo's bar):** a stub HTTP server answering `/api/agent/ask` with `{"ok":true,"result":"..."}` — exactly the pattern of the `resolve_local_model` tests added this session (tokio listener + canned response). Assert: (1) the happy path returns the result text; (2) a 502/connection-refused produces `Error::Network`, never a fabricated reply; (3) a missing token is `InvalidConfig`. Full suite + clippy + fmt must stay green; the live-model CI check must still complete a real conversation (it will, since ABP is only reached when configured).

**Effort:** ~1 week including tests. **Risk:** low-medium (new outbound dependency; the degradation contract above is the whole safety story).

### 4.4 Pattern D — ScreenBuddy as a first-class ABP backend kind (~2 weeks, optional)

**Mechanism.** Follow `bot/backends/external_agent_backend.py` exactly: a new `backends/screenbuddy_backend.py` implementing `async def ask(prompt, *, context=None, timeout_s=30) -> BackendResult`, run in a `process_cell` (sandbox_ns; a Win32 Job Object so one kill takes the tree), capturing a session key for cross-call continuity. ScreenBuddy-side it needs a thin CLI or IPC client: `send_chat` → poll `get_chat_history` for the newest assistant message (the same transcript-reading pattern `tools/live_desktop_check.py` already uses — including its UTF-8 stdout fix, since replies contain emoji).

Two registration options:
- **Built-in-style:** add to `BUILTIN_BACKENDS` in `bot/router.py` (touches core — needs a PR).
- **Extension (zero core edits):** a local extension calling `add_backend_class("screenbuddy", factory)` — hot-reloadable, matches the documented extension pattern. **Recommended.**

**Result.** A Telegram/Discord bot whose *engine* is the ScreenBuddy creature — full personality, memory, animations, sounds — while ABP keeps channels, pairing, schedules, dashboards, and the learning router picks which model drives it.

**Effort:** ~2 weeks. **Risk:** medium (process lifecycle, session-key persistence, kill semantics — all solved patterns in `external_agent_backend.py`, but ScreenBuddy's IPC has no streaming/cancel; the poll loop must handle the ~60s generation window the live check exercises, with `timeout_s` enforced inside the cell).

**Design note:** if this backend drives small local models, ABP's LF2 small-model mode (`small_model.py`) will trim its tool set automatically — a nice pairing with ScreenBuddy's `LocalTiny` default tier.

### 4.5 Pattern E — ScreenBuddy consumes ABP's MCP server (~2–3 weeks, optional)

**Mechanism.** ScreenBuddy's `register_tool` is the extension point, but there is no Rust MCP client in the workspace today — this is net-new code (a minimal stdio JSON-RPC client: `initialize` → `tools/list` → `tools/call`, mirroring what `mcp/server.py` does in reverse). Tools worth surfacing first: `ask_instance` (agent-to-agent), `run_swarm`, `list_jobs`, `create_snapshot`.

**Result.** The creature orchestrates the fleet: "ask the research bot to…", "run a fanout swarm on…". Most powerful, most expensive — **defer until A–C are stable.**

### 4.6 Pattern F — Unified Android control (small, optional)

ScreenBuddy's 27 broadcast commands vs ABP's Android app (pairing codes, FCM, Support tab). Either app gains a panel for the other. Low priority vs A–C.

---

## 5. Architecture rationale — why presence layer, not backend swap

1. **ScreenBuddy's identity is the product.** Creature, animations, sounds, overlay, single-user intimacy — integration must preserve these. Patterns A/C *add* reach; Pattern D risks turning the creature into a headless engine behind a chat channel (still a valid option, just a different product decision).
2. **ABP's value is routing + fabric, not personality.** Learned routing, backup chains, swarms, multi-channel — invisible to the user, accretive to ScreenBuddy.
3. **Trust boundaries already align.** Both localhost-first; ScreenBuddy IPC has optional token auth; ABP's MCP client is operator-configured; ABP's sandbox/taint system is exactly what ScreenBuddy's minimal loop lacks — so ScreenBuddy's requests *through* ABP inherit ABP's safety machinery (estop, sandbox, taint, secrets guard, net allowlist), while ABP's tools *inside* ScreenBuddy stay bounded by ScreenBuddy's surface.
4. **The routing stacks, not conflicts.** 9Router = provider-level fallback inside one endpoint (ScreenBuddy-side, static). ABP router brain = model-level selection across endpoints (learned). Running both is strictly better than either alone.
5. **One hazard to respect:** ABP's README warns never to configure the same platform bot token in both Hermes's gateway and an ABP instance — one owner per token. ScreenBuddy owns no platform tokens today; if it ever does, the same rule applies.

**End-state data flow:**

```
Telegram/Discord/Slack ──► ABP bot instance ──► Router.ask() (learned policy,
                                   │              backup chain, e-stop)
                                   │                    │
                                   │                    ├─► ScreenBuddy 9Router (:20128)   [Pattern B]
                                   │                    ├─► Anthropic / OpenAI / Ollama
                                   │                    └─► ScreenBuddy gen_abp ────────── [Pattern C]
                                   │                              │
                                   │                              ▼
                                   │                    ScreenBuddy creature (persona, memory,
                                   │                    animations, sounds) answers
                                   │
                                   └─► ABP agent tools ──► ScreenBuddy MCP (25 tools)   [Pattern A]
```

---

## 6. Security & operations (verified)

- **ScreenBuddy IPC**: `127.0.0.1:34567`, optional `auth_token` (first-message `{"auth":...}`). Set it when ABP connects via stdio MCP on a shared machine.
- **ABP dashboard**: `X-Dashboard-Token` (or API key); per-instance `allowed_user_ids`; destructive actions confirm-gated (in-memory 5-min tokens).
- **ABP MCP client trust**: operator-configured only, never agent-creatable — the ScreenBuddy registration inherits this.
- **ABP sandbox does not cover ScreenBuddy's tools**: same caveat as `opencode`/`openclaw`. `outbox._refuse_in_sandbox` shows ABP takes this boundary seriously elsewhere — mirror that discipline on the ScreenBuddy side if tools are exposed to untrusted channels (per-profile `tools_enabled` already exists).
- **ABP's estop** blocks new turns at the single `Router.ask` entry point — a global kill switch that also covers ScreenBuddy-originated asks via Pattern C. Worth surfacing in ScreenBuddy's settings UI eventually.
- **API keys**: ScreenBuddy reads `OPENAI_API_KEY`/`ANTHROPIC_API_KEY` from env; ABP from git-ignored `.env`. Never commit either. ABP's provider store keeps keys encrypted with the vault key and never returns them via the API.

---

## 7. Gap analysis — what each side gives and gets

| Capability | ScreenBuddy today | ABP today | After integration |
|---|---|---|---|
| Channels beyond desktop | — | 4 platforms + app channel | Creature reachable from any channel (A/D) |
| Learned model routing | static tier selection | router brain + cooldowns | Creature asks inherit it (C) |
| Backup chains | per-request retry only | backend + model fallback | Creature asks survive provider death (C) |
| Agentic tools (shell/git/sandbox) | echo, time | ~57 modules | Creature's prompts gain them via custom_model loop (B) / ABP engine (C/D) |
| Multi-agent orchestration | — | swarms (5 strategies), agent-to-agent | Creature can fan out (E, later) |
| Desktop presence / personality | full | none | ABP bots get a face (A/D) |
| Memory/RAG | memory_ingest/search | context vault, session linking | Bridge both directions (future) |
| Small-model handling | — | LF2 trimmed turns | Local models behave well behind ABP (B/D) |

---

## 8. Failure-mode contract (the non-negotiable rules)

Both codebases share one hard invariant, learned the hard way this session: **a failure must be reported, never papered over with a plausible-looking reply.** Every integration point must honor it:

1. ABP unreachable / token rejected / 5xx → `Error::Network` with status + body. The existing `send_message` path turns that into "I could not reach the AI model: …" — an honest failure, shown to the user.
2. 9Router model missing → `resolve_local_model` falls back to an installed model (already fixed); if none installed → explicit "no models installed" error.
3. MCP server down → ABP's client omits that server's tools and logs once; it never breaks the turn (documented ABP behavior — verify it holds for the ScreenBuddy registration).
4. Timeout → ABP's cell kill (Win32 Job Object) takes the process tree; ScreenBuddy-side poll loops must enforce `timeout_s` and surface it.
5. No integration path may ever route a failure into a canned/fallback reply. The v0.6.3 CI gate (live-model check: "the reply came from the model, not a fallback") is the enforcement mechanism — keep it green through every phase.

---

## 9. Recommended sequencing

| Phase | Pattern | Effort | Value | Prereq |
|---|---|---|---|---|
| 1 | B — 9Router as ABP provider | config only | High — instant routing gain | 9Router running |
| 2 | A — ABP MCP client ← ScreenBuddy | config only | Highest — 25 tools to every agent turn | ScreenBuddy app + MCP server |
| 3 | C2 — ScreenBuddy `Backend::Abp` | ~1 week | High — creature becomes fleet front-end | Phase 1; ABP dashboard token |
| 4 | D — ScreenBuddy as ABP backend (extension) | ~2 weeks | Medium — channel bots powered by the creature | Phase 3 stable |
| 5 | E — ScreenBuddy Rust MCP client | ~2–3 weeks | Medium-high — creature orchestrates fleet | Phase 4 |

**Do 1+2 today** — configuration, reversible, immediately demonstrable. **Phase 3 is the strategic one**: ScreenBuddy stops being "an app with 6 backends" and becomes "the presence layer of an agentic fleet" without losing its identity.

**Definition of done per phase** (ScreenBuddy's established bar): new backend variants ship with failing-then-passing tests against a stub server (the `resolve_local_model` tests are the template); 391 lib + 181 bin tests stay green; `clippy --all-targets -D warnings` and `cargo fmt` clean; live-model CI still completes a real conversation; and every failure path reports honestly per §8.
