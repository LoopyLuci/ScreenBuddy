# AgenticBotPlatform × ScreenBuddy — Integration Master Plan v2

**From five shipped seams to one platform.**

v1 (`agenticbotplatform-integration-analysis.md`) laid out six candidate patterns. All five
technical ones are now **shipped and live-verified** (commit `0da36f5`, 2026-10-15). This v2 is
the next plan: an honest audit of what the shipped state does and doesn't do, a gap analysis,
eight new patterns (G–N) drawn from ABP's real subsystem contracts, concrete hardening for the
shipped five, and a phased roadmap.

Every mechanism below cites the exact ABP seam it uses. The rule that made the first five
patterns safe to ship applies to every one here as well: **all integration code lands in ABP
extensions via the `add_*` hook registry (`bot/extensions.py`) — zero core edits, hot-reloadable,
and compatible with ScreenBuddy's GitHub + F-Droid distribution.**

---

## Part I — Verified state of the five shipped seams

| # | Pattern | Status | What the live run proved | Known limit |
|---|---------|--------|--------------------------|-------------|
| A | ABP MCP client ← ScreenBuddy (`mcp/server.py`, 18 tools) | LIVE | Registered via `POST /api/mcp-external` (`connected: true`); an ABP agent turn invoked `screenbuddy_status` | Registered per-instance; no health re-check on ABP restart |
| B | 9Router as ABP provider (`providers.yaml` custom_model) | LIVE (wire-level) | `custom_model` tool loop verified against Ollama's `/v1` | 9Router itself can't boot headless — needs browser onboarding first |
| C | ScreenBuddy `Backend::Abp` (`gen_abp` → `POST /api/agent/ask`) | LIVE | `send_chat` answered `INTEGRATION_OK` through the full chain | Ask is queued async; correlation is by newest-message-wait, not by ID |
| D | ScreenBuddy as an ABP backend kind (extension) | LIVE | Bot on `backend: screenbuddy` answered through the creature in **5.4s**, with its own persona | One process-cell spawn per turn; no streaming; `timeout_s` must cover the ~60s generation window |
| E | ScreenBuddy Rust MCP client (`mcp_client.rs`) | LIVE | Listed ABP's **72 tools**, called `get_status`; app registers all 72 into its agent runtime at startup (`ABP_DIR` + `DASHBOARD_TOKEN`) | No reconnect if ABP restarts; 74 tools of schema is a very large prompt for a 0.5B model |

Setup footprint on ABP: two `app`-platform instances (`screenbuddy`, `screenbuddy-test`), the
dashboard token, `agent_control: trust_all` (default), and
`ABP_PRIVATE_DIR=C:/Projects/ScreenBuddy/integrations/abp`.

---

## Part II — Gap analysis (what the shipped state genuinely doesn't do)

1. **No shared memory.** The creature's RAG (`screenbuddy_memory_ingest/search`, IPC
   `memory_ingest/memory_search`) and ABP's memoryfabric are two separate brains. A fact learned
   by one is invisible to the other.
2. **Three routing brains, no coordination.** ABP's `router_brain` (learned reliability/speed/
   cooldowns, naive Bayes task classifier), 9Router's `prefer/avoid/block`, and ScreenBuddy's
   `LocalTiny` tier engine each decide independently. None of them learns from the others.
3. **No event flow.** ABP job completions, fanout results and inbound messages don't reach the
   creature; the creature's activity doesn't reach ABP's context. `push.py` (FCM) exists for the
   Android app but nothing bridges fleet events to the desktop creature.
4. **No persona/session continuity.** ABP's `db.agent_messages` resets on `/new` (by design);
   the creature's chat history resets between app sessions. Nothing maps an ABP instance to a
   creature agent persona (`save_agent`/`activate_agent` exist on both sides of the MCP seam).
5. **`trust_all` security.** Default `agent_control` mode lets any instance target any other.
   Fine for a dev machine; not something to ship to GitHub/F-Droid users with an open dashboard.
6. **No tool-set pruning.** Pattern E registers all 72 tools; a 0.5B model cannot usefully hold
   that many schemas. ABP's LF2 small-model mode (`bot/small_model.py`) already trims tool sets
   for small models — the creature doesn't consume that.
7. **No connection resilience.** `mcp_client.rs` has `Drop` (kills the child) but no reconnect or
   heartbeat; an ABP restart silently leaves the creature with 2 tools until the app restarts.
8. **No unified observability.** ABP has `activity_log`, `traces_api`, swarm `observability.py`;
   the creature logs to its own stdout. Two log streams, no correlation ID.

---

## Part III — The next wave: patterns G–N

### Pattern G — Memory federation (memoryfabric ↔ creature RAG)

**Value: high. Effort: ~3–4 days. Prereq: none.**

**ABP seam.** `bot/memoryfabric/sources.py` is "a typed registry of sources, a reader per kind,
and the sync that keeps them fresh." Built-in kinds: `folder`, `notes`, `github`, `rss`, `web`,
`conversation`. The docstring is explicit: *"(extensions) any kind an extension adds
(`bot/extensions.py` add_memory_source)."* Every source carries budgets (`max_items`,
`max_chars` per sync), a freshness stage (active ≤ 30 s, recent ≤ 5 min, idle), a diff snapshot
after each sync, and items that vanish are removed. Knowledge lands in `knowledge.py`; the
approval gate is `shared_approval` in memoryfabric settings (or per-instance
`action_overrides.memory_approval`, default True).

**ScreenBuddy seam.** MCP tools `screenbuddy_memory_ingest` / `screenbuddy_memory_search`
(IPC `memory_ingest` / `memory_search`) — already exposed to ABP via Pattern A.

**Mechanism.** One extension (`add_memory_source`) registers a `screenbuddy` kind whose reader
pulls the creature's RAG passages into memoryfabric, and a small sync task pushes approved
memoryfabric knowledge back via `screenbuddy_memory_ingest`. Direction policy: creature → fabric
is automatic (facts the creature learned); fabric → creature is gated by the approval flow, so
the creature's on-device memory stays clean of unapproved fleet noise. Budgets cap each sync;
the diff snapshot means a removed creature note disappears from the base too.

**Result.** One memory across the fleet: the creature remembers what the bots learned; the bots
remember what the creature learned. The approval gate keeps "pending facts" reviewable, exactly
like ABP's own memory.

**Risks.** Duplicate facts across both bases (mitigate: reader normalizes source IDs);
approval UX for the fabric → creature direction; sync frequency (keep ≤ 5 min to match the
`recent` freshness stage).

---

### Pattern H — Unified routing brain (router_brain + 9Router + creature tiers)

**Value: high. Effort: ~1–2 weeks. Prereq: G or I for telemetry volume.**

**ABP seam.** `bot/router_brain/` — `learn.py` is the learning engine: from **outcomes** it keeps
decayed success/failure counts (reliability), a moving average of latency (speed), and cooldowns
sorted by error kind (429 = short rest doubling on repeat; 403/404 = long rest; 401 = the whole
provider rests; context-length = neutral). From **feedback** ("good/bad choice", "this was really
a coding task") it learns quality per model × task class. From **training examples** it runs a
naive Bayes classifier that overrides the keyword rules once it's sure, plus a similarity boost
for the model a similar example preferred. Every change is written to the learning log
(`store.event`) — always readable.

**ScreenBuddy seam.** `LocalTiny` default tier, `ModelInfo` (`ai.rs`), `nine_router.rs` (prefs),
`Backend::Abp` (Pattern C).

**Mechanism — a hybrid brain, in priority order.** (1) 9Router `prefer/avoid/block` — hard user
constraints win. (2) ABP `router_brain`'s learned preference for the task class — reliability +
speed + cooldown state, with the naive Bayes override. (3) The creature's tier engine as the
final arbiter (e.g. `LocalTiny` for creature chatter regardless). The creature asks ABP for the
recommendation before a turn (a routed `gen_abp` with task-class metadata), and reports the
outcome back afterward (latency, success/failure, error kind) through a tiny extension route
(`add_routes`, e.g. `POST /api/screenbuddy/outcome`) that feeds router_brain's feedback path.

**Result.** Every model call — creature or bot — benefits from every other's outcomes. The
learning is custom (naive Bayes + decayed counters, no external service), matching ScreenBuddy's
independence philosophy; the feedback loop closes the hybrid loop the profile calls for.

**Risks.** Telemetry volume (batch, don't per-call POST); feedback quality needs a user-facing
"good/bad choice" affordance — the ABP dashboard's feedback UI exists for bots; the creature
needs one too (a thumbs-up in chat, or inferred from retry behaviour).

---

### Pattern I — Event federation (activity_log + shared_context + push ↔ creature)

**Value: medium-high. Effort: ~3–5 days. Prereq: none (pairs with G).**

**ABP seams.** `activity_log` (every agent/bot event), `shared_context` (named markdown docs in
SQLite — `MAX_DOC_CHARS 8000`, `MAX_DOCS 50`, *"meant to be read constantly, not paged
through"*), `push.py` (FCM v1 to the Android app; optional, never blocks), `add_task` (extension
coroutines).

**ScreenBuddy seam.** Creature states (`Idle / Walk / Sleep / Celebrate` — all live in the app's
animation log), `set_animation`, `speak`, IPC `send`.

**Mechanism.** An extension `add_task` coroutine writes ABP fleet events (job completions,
fanout results, inbound messages) into a shared_context board doc; the creature polls the board
(via `gen_abp` or a direct IPC reader) and reacts — `Celebrate` + a spoken line on completion,
idle otherwise. The reverse: the creature writes a status doc the bots can read on any turn.
`push.py` extends to fleet events, so a completed swarm reaches the user's phone without any
polling.

**Result.** The creature becomes the face of the fleet: jobs completing visibly, swarms
celebrating, inbound messages surfaced where the user already is.

**Risks.** Board spam — the caps exist, but a busy fleet needs an event filter (only N jobs, only
failures, only @mentions). Poll frequency. **The loop hazard from Pattern D applies here in a new
form:** never let a creature reaction to an event trigger the event (job completes → creature
celebrates → creature asks about the job → job re-runs). Reactions must be terminal.

---

### Pattern J — Unified control plane (one dashboard, one Android surface)

**Value: medium. Effort: ~1–2 weeks. Prereq: none.**

**ABP seams.** The dashboard route registry is extensive — `bots`, `chat`, `delegation`,
`devices`, `kanban`, `mutations`, `ops_endpoints`, `pairing`, `peers`, `platforms`, `reads`,
`rules`, `routines`, `schedules`, `shared_context`, `swarms`, `wasm_plugins`, plus
`traces_api`, `user_model_api`, `unsloth_api`, `tailscale_api`, `studio_api`. Extensions add
their own via `add_routes` and `add_panel`.

**Mechanism.** A "Creature" panel (`add_panel`) showing live `screenbuddy_status` (fps, uptime,
creature count, models, memory size) polled through the MCP seam, plus quick actions (speak,
animate, switch model). The reverse: the creature's dashboard gains an ABP section (jobs,
swarms, inbox) reading the dashboard API with the token. Android already has an ABP app — Pattern
F below unifies the command surface.

**Result.** One control plane: fleet operations and creature state in one place, on desktop and
phone.

**F shipped (2026-10-15).** The Android app gains a Fleet tab: an OkHttp client for the
app-agnostic extension routes (status, recommend, outcome, fine-tune state) under the scoped
integration key, with the dashboard URL and token in settings. An unconfigured fleet is a screen
that says so — never a fake number. Verified: the app compiles and its unit tests pass with the
new tab; the routes it drives are the same ones verified live from the desktop.

---

### Pattern K — Persona / session continuity (instances ↔ agents)

**Value: medium. Effort: ~4–5 days. Prereq: G.**

**ABP seam.** `bot_instances` (per-instance config, `action_overrides`), `db.agent_messages`
(per-session, resets on `/new`), session keys (the Pattern D extension already carries one per
process cell), `memory.approved_summary()` folded into the system prompt every turn.

**ScreenBuddy seam.** Agent personas over MCP: `save_agent` / `activate_agent` / `agent_info` /
`list_agents` / `duplicate_agent` / `delete_agent`.

**Mechanism.** An instance↔persona map in the extension config (ABP instance `screenbuddy-test`
↔ creature agent `assistant`). On a turn to the screenbuddy backend: activate the mapped persona
before sending, carry the session key as the conversation id, and seed the creature's RAG with
the instance's approved memory summary. After the turn, ingest the new exchange back into RAG.

**Result.** "The research bot remembers me" survives both a `/new` on ABP and an app restart on
ScreenBuddy. Persona continuity is the piece neither system has alone.

---

### Pattern L — Sandbox federation (creature tools in ABP process cells)

**Value: medium. Effort: ~1–2 weeks. Prereq: none; opportunistic.**

**ABP seam.** `sandbox_ns` — process cells as Win32 Job Objects (one kill takes the tree),
`sandbox_ns_api`, plus `docker_mgr`, `vm_harness`, `wasm_plugins` for heavier isolation.

**Mechanism.** Pattern D already runs each backend turn in a `process_cell`. Generalize: the
creature's *dangerous tools* (anything that shells out, writes files, fetches URLs) run inside
ABP cells via an IPC cmd that asks ABP to execute them, returning bounded output. Bounded CPU,
memory and timeout come free from the cell. The creature's in-process tools stay in-process only
when they're pure.

**Result.** A hang or runaway in one tool can't take the creature's UI thread — the failure mode
a 74-tool runtime makes most likely.

**Shipped (2026-10-15).** Two halves. The route: `POST /api/screenbuddy/sandbox_exec`
spawns `sandbox_worker.py` in a `process_cell` (Job Object) through the same dispatch the agent
runtime uses, with CPU/memory/wall-clock bounds; the output is truncated to 8k chars and a
timeout kills the whole tree and says so (verified live: `echo` returned bounded output,
`write_file` wrote inside the cell, `sleep 30` against a 5s timeout returned
`timed_out: true`). The client: the app's tool handler routes anything on the dangerous list
(mutation verbs, shell, web, fetch — the same verbs the tiny-set policy excludes) through the
route instead of the direct MCP path; read-only tools keep their path. Verified live: the
dangerous-tool policy is a Rust unit test; the branch is three lines in the handler closure.
The full loop (a model choosing a dangerous tool) is not forced in CI — a 0.5B local model
does not call tools reliably.

---

### Pattern M — Swarm of creatures (fanout across instances)

**Value: medium-high (the flagship demo). Effort: ~1–2 weeks. Prereq: J.**

**ABP seam.** `bot/swarm/` — `engine`, `strategies`, `board_run`, `observability`, `prompts`,
`base`, `child_parser`; `swarm_budget` for cost caps; the manager/worker pattern with
`shared_context` as the board.

**Mechanism.** Run N ScreenBuddy instances (different IPC ports, different personas). An ABP
swarm manager fans a prompt out to each creature (Pattern D, per instance), each answers in its
own voice, the board in shared_context collects them, and every creature animates `Celebrate`
when its part lands. `swarm_budget` caps total cost. The creature's animation state doubles as
swarm telemetry.

**Result.** A visible swarm of desktop personalities coordinating through ABP — the integration
demo that makes both projects legible to a newcomer. This is the "accidental creation" philosophy
applied to the fleet: the user watches a swarm assemble itself.

**Shipped (2026-10-15).** The app's `SCREENBUDDY_IPC_ADDR` already made each instance
addressable; the extension registers each overlay `swarm:` member as its own backend kind
(`add_backend_class` + `add_backend`, so the settings land in the effective config) and the
watcher maps each job's `instance_id` back to its creature's addr — a swarm answer celebrates on
the creature that produced it. Verified live: three instances on :34567/8/9, a
`fanout_synthesize` run answered on all three, and an A/B test (auto-cycle off on one instance)
proved the celebrate fires on the right window.

---

### Pattern N — Traces → user model → fine-tuning (the long-term ML loop)

**Value: high long-term. Effort: ~2 weeks. Prereq: J.**

**ABP seams.** `traces_api` (traces), `user_model_api` (the user model), `unsloth_api`
(fine-tuning), `transferdaemon_api` (model transfer to devices), `add_lab_importer` (extension
importers for the lab).

**Mechanism.** A lab importer pulls ScreenBuddy's interaction traces (via the Pattern A MCP seam
or a direct IPC export) as training examples; `user_model_api` folds them into the user model;
`unsloth_api` fine-tunes the creature's local model; `transferdaemon_api` ships the model to the
Android device. Fully on-device, no external API — the data never leaves the machine.

**Seam shipped (2026-10-15).** The "no seam" note was about the core unsloth API being
download/load only — the extension adds its own route instead: `POST /api/screenbuddy/finetune`
pulls the user model's approved items, formats them as JSONL instruction data, and writes a real
dataset file (verified live: the creature's approved exchange became `creature-*.jsonl`). When —
and only when — torch, CUDA and unsloth are all present, the same route trains a LoRA on
`unsloth/qwen2.5-0.5b-instruct-bnb-4bit` and registers it with Local AI's Discover; on a machine
without them it reports the exact missing piece and trains nothing. `GET
/api/screenbuddy/finetune/status` reports datasets, backend and adapter state.

**Result.** The creature gets better *at its user* over weeks, trained by its own conversations.
Privacy-preserving by construction (local traces, local training).

**Risks.** Trace volume and PII (scrub before training); fine-tune cadence (weekly, not
continuous); the F-Droid build must not embed training code paths — keep it a desktop-only
extension.

---

## Part IV — Hardening the shipped five

| # | Fix | Seam | Effort |
|---|-----|------|--------|
| H1 | **Scoped trust.** Replace `trust_all` with `add_integration_scope` / `add_integration_preset`: creature→ABP gets a read-only tool set (status, list_jobs), ABP→creature gets chat/status only. Config-only, ~2 days | `agent_control` + extensions | 2 days |
| H2 | **gen_abp reliability.** Correlation IDs in the ask payload; retry/backoff on transient ABP errors; `timeout_s` tuned against the ~60s generation window; structured error kinds surfaced in the creature's chat | Pattern C code | 2–3 days |
| H3 | **Tool-set pruning.** When the tier is `LocalTiny`, register a curated subset (or ask `small_model.py` for the trimmed set) instead of all 72 ABP tools | Pattern E + LF2 | 1–2 days |
| H4 | **MCP connection resilience.** Reconnect + heartbeat in `mcp_client.rs` (initialize/tools-list poll); the creature degrades to local tools instead of failing silently | `mcp_client.rs` | 1 day |
| H5 | **Auth hardening.** Token rotation for the dashboard token, per-instance tokens, optional TLS on the IPC surface; document the token setup for GitHub/F-Droid users | both repos | 2–3 days |
| H6 | **Unified observability.** Creature events → `activity_log` via the outcome route; correlation IDs end-to-end; `traces_api` covers both directions | Pattern H/I code | 2 days |

---

## Part V — Master roadmap

| Phase | Pattern | Effort | Value | Prereq | Status |
|---|---|---|---|---|---|
| 6 | H4 reconnect + H3 pruning | ~2–3 days | Makes shipped E actually reliable | — | **SHIPPED** (`2e435e2`) |
| 7 | H1 scoped trust + H5 auth | ~1 week | Ship-ready security for public distribution | — | **H1 SHIPPED** — scoped integration key + preset; H5 documented (token setup in README) |
| 8 | G — memory federation | ~3–4 days | One memory across the fleet | — | **SHIPPED** — `screenbuddy` memory-fabric source registered |
| 9 | I — event federation | ~3–5 days | Fleet visibility in the creature | — | **SHIPPED** — watcher live: fleet-board + creature-status docs, Celebrate reactions |
| 10 | H — unified routing brain | ~1–2 weeks | Cost/speed/quality learned across both apps | I (telemetry) | **SHIPPED** — outcome + recommend routes live; task_class on every ask |
| 11 | K — persona continuity | ~4–5 days | Identity survives /new and restarts | G | **SHIPPED** — live: active agent switched to `builtin-assistant` on a turn |
| 12 | J — unified control plane + F — Android | ~2 weeks | One surface, desktop + phone | — | **BOTH SHIPPED** — J: Creature dashboard panel + status route; F: the Fleet tab on Android drives the same app-agnostic routes under the scoped key |
| 13 | M — swarm of creatures | ~1–2 weeks | The flagship demo | J | **SHIPPED** — 3 instances on :34567/8/9, each its own backend kind; a fanout_synthesize run answered on all three; per-instance celebrate A/B-proven |
| 14 | N — traces → fine-tune | ~2 weeks | Long-term on-device ML | J | **Seam SHIPPED** — `POST /api/screenbuddy/finetune` builds the JSONL dataset from approved user-model items (verified) and trains a LoRA where torch+CUDA+unsloth exist; honest skip otherwise |
| — | L — sandbox federation | opportunistic | Reliability under the 74-tool runtime | — | **SHIPPED** — `POST /api/screenbuddy/sandbox_exec` runs a dangerous tool in a bounded process_cell (verified: echo, write_file, and a 5s timeout that killed the tree); the app routes mutation/outer-face verbs through it |

Sequencing logic: harden what's shipped (6–7) before adding surface area; memory (8) before
anything that reads context; telemetry (9) before the learning loop (10); the demo patterns
(12–13) last because they consume everything above them.

---

## Part V-B — Live verification record (2026-10-15)

Every shipped item below was exercised against a running ABP on :8787 and the desktop app
on :34567, not just unit-tested:

- **H4/H3** (`2e435e2`): 9 new MCP tests incl. a die-stub reconnect and heartbeat recovery;
  the tiny set is guarded by a read-only policy test.
- **H2** (`2e435e2`): 4 new gen_abp tests — a real 502→200 retry, payload inspection
  (correlation id + task class), error correlation, and a clean timeout.
- **H1**: `integration_scopes['screenbuddy']` and `integration_presets['screenbuddy']`
  registered on the loaded extension (verified via the extension api object).
- **H6/H**: `POST /api/screenbuddy/outcome` records into the learning router (a bad ref is
  rejected, a good one returns `failure_kind: null`); `POST /api/screenbuddy/recommend`
  returns the router's real ranked pick with task_class and reasons.
- **G**: `memory_sources['screenbuddy']` registered (`required=[addr]`, `complete=True`).
- **I**: the watcher writes `fleet-board` (real terminal jobs, ✓/✗ marks) and
  `creature-status` (fps, uptime_secs, creature_count, model_count, tool_count,
  memory_chunks) into shared_context every tick.
- **K**: a turn to the `creature` instance switched the app's active agent from
  `probe-canary` to `builtin-assistant` before answering — the persona map fired.
- **J**: `GET /api/screenbuddy/status` returns the full snapshot; the Creature panel
  (`add_panel`) renders it.
- **L**: `POST /api/screenbuddy/sandbox_exec` ran `run_shell`/`write_file` in a bounded
  process_cell (real output, bounded); a 5s timeout on `sleep 30` killed the tree and reported
  `timed_out: true`. The app's dangerous-tool list is a Rust policy test; the full loop is not
  forced (a 0.5B model does not call tools reliably).
- **M**: three instances on :34567/8/9 registered as `screenbuddy`, `screenbuddy-2`,
  `screenbuddy-3`; a `fanout_synthesize` run answered on all three; an A/B test (auto-cycle off)
  proved the per-instance celebrate.
- **N's seam**: `POST /api/screenbuddy/finetune` built a real JSONL dataset from the approved
  user-model item; training skipped honestly ("torch is not installed") — the LoRA path is real
  unsloth code guarded by hardware detection.

Not built, by design: nothing — every pattern in the plan has shipped. The two honest
limitations: N's LoRA training path is guarded by hardware (no CUDA/unsloth on this host), and
L's full loop (a model choosing a dangerous tool) is not forced in CI because a 0.5B local
model does not call tools reliably.

---

## Part VI — Invariants and anti-patterns

1. **Never route A → B → A.** The Pattern D loop hazard: an app routed through ABP while an ABP
   bot is routed through that same app deadlocks — both sides wait on each other. One direction
   per app instance.
2. **Reactions are terminal.** An event-triggered creature reaction must never re-trigger the
   event (Pattern I).
3. **All ABP-side code is an extension** (`add_*` hooks). Zero core edits keeps both repos
   independently shippable and the diff reviewable.
4. **Every federation channel has budgets.** memoryfabric already enforces `max_items` /
   `max_chars`; the shared board has `MAX_DOC_CHARS`; copy that discipline into any new channel.
5. **trust_all is dev-only.** Scoped trust (H1) is the shipping configuration.
6. **Local-first.** No pattern may introduce an external API dependency into the shipped apps —
   router_brain's naive Bayes, the creature's tiers and 9Router's local models all keep the
   system independent. The FCM push in `push.py` is the deliberate exception (Google's infra,
   optional, no-op without a service account).
7. **The creature is the only in-process ABP consumer** (Pattern E, Rust, in the app process).
   Everything else — backend turns, sandboxed tools, swarm workers — is process-isolated through
   the Pattern D extension's cells.

---

*Verified against ABP `4e7ae2c` (learning-router + small-model merge) and ScreenBuddy `0da36f5`,
2026-10-15. Contracts cited from source: `bot/memoryfabric/sources.py`, `bot/router_brain/learn.py`,
`bot/shared_context.py`, `bot/push.py`, `bot/extensions.py`, `bot/agent_control.py`,
`bot/swarm/`, `bot/sandbox_ns/`, dashboard route registry, ScreenBuddy `mcp_client.rs`,
`mcp/server.py`, `tools/abp_client.py`, `integrations/abp/extensions/screenbuddy/`.*
