# ScreenBuddy Settings Reference

## Live Settings

These settings take effect immediately when changed.

| Setting | Type | Range | Default | Description |
|---------|------|-------|---------|-------------|
| `animation_speed` | Float | 0.1–3.0 | 1.0 | Creature animation playback speed |
| `fps_target` | Integer | 5–120 | 60 | Target frame rate for the render loop |
| `rag_enabled` | Boolean | true/false | true | Enable RAG memory ingest and search |
| `volume_master` | Float | 0.0–1.0 | 0.8 | Master audio volume |
| `volume_effects` | Float | 0.0–1.0 | 1.0 | Sound effects volume |
| `volume_music` | Float | 0.0–1.0 | 0.5 | Background music volume |
| `ai_provider` | String | — | "ollama" | AI provider identifier |
| `ai_model` | String | — | (provider default) | AI model name |
| `window_transparency` | Float | 0.1–1.0 | 1.0 | Window alpha (0.1 minimum to keep creature visible) |
| `always_on_top` | Boolean | true/false | false | Keep window above all others |

## Read-Only Settings

These reflect real runtime state and cannot be edited.

| Setting | Type | Description |
|---------|------|-------------|
| `collision_avoidance` | Boolean | Whether creature-creature collision is active |
| `creature_count` | Integer | Number of creatures currently loaded |
| `cursor_interaction` | Boolean | Whether the creature responds to cursor proximity |

## Unwired Settings

These are visible in the settings window but have no effect. They are
clearly labelled and documented in source.

| Setting | Reason |
|---------|--------|
| `multi_gpu` | The desktop app renders through GDI. `MultiGpuRenderer` enumerates real wgpu adapters but is never constructed. |
| `thread_pool_size` | `ThreadPool::new()` takes no size and the render loop is single-threaded. |

## AI Provider Settings

| Setting | Type | Description |
|---------|------|-------------|
| `ai_provider` | String | Provider identifier (e.g., "ollama", "openai", "anthropic") |
| `ai_model` | String | Model name (provider-specific) |
| `ai_api_key` | String | API key (stored encrypted) |
| `ai_base_url` | String | Custom base URL for compatible providers |
| `ai_temperature` | Float | Sampling temperature (0.0–2.0) |
| `ai_max_iterations` | Integer | Maximum tool-call iterations (1–20) |
| `ai_timeout_secs` | Integer | Request timeout in seconds (5–300) |

## Agent Settings

Each agent profile has its own settings, editable in the Agent Editor:

| Setting | Type | Description |
|---------|------|-------------|
| `name` | String | Agent display name |
| `persona` | String | Agent personality description |
| `instructions` | String | System instructions for the agent |
| `character` | String | Associated creature ID |
| `movement_speed` | Float | How fast the creature moves |
| `autonomous_actions` | Boolean | Whether the creature acts on its own |
| `tools_enabled` | Boolean | Whether the agent can use tools |

## 9Router Settings

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `nine_router_endpoint` | String | `http://localhost:20128/v1` | 9Router API endpoint |
| `nine_router_api_key` | String | (empty) | Optional API key for 9Router |

## Persistence

Settings persist on every change to `%APPDATA%\ScreenBuddy\settings.json`.
No save button is needed. Out-of-range values are refused with a reason
and the previous value is kept.

## Import/Settings

Settings profiles can be exported and imported:

- **Export**: Settings → Export → choose a file location
- **Import**: Settings → Import → select a previously exported file

Profiles are JSON files containing all live and AI provider settings.
