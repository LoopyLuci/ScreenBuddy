# ScreenBuddy Project

A next-generation desktop AI companion platform.

## Project Structure

```
ScreenBuddy/
├── Cargo.toml              # Rust workspace root
├── toolkit/
│   ├── bin/
│   │   ├── Godot_v4.3-stable_win64.exe
│   │   ├── Godot_v4.3-stable_win64_console.exe
│   │   └── godot-harness.sh
│   └── godot-project/      # Godot project for creature creation
│       ├── project.godot
│       ├── creatures/      # Creature definitions (JSON)
│       ├── scenes/         # Godot scenes
│       ├── scripts/        # GDScript files
│       └── assets/         # Textures, sounds, fonts
├── crates/
│   └── screenbuddy-core/   # Rust core library
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── creature.rs
│           ├── state.rs
│           ├── platform.rs
│           ├── render.rs
│           └── error.rs
├── exports/                # Exported creatures
└── docs/                   # Documentation
```

## Quick Start

### Build (Windows)

The Rust workspace targets Windows and requires a working Rust toolchain.

```bash
# Build + test the Rust workspace
cargo build --workspace
cargo test --workspace

# Lint gates (these are what CI enforces)
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

### Run

```bash
cargo run --bin screenbuddy
```

The app starts, loads its 7 creatures, and runs the animation loop at 30 FPS.

### Android (APK)

Requires **JDK 17** (AGP 8.2.0's `JdkImageTransform` fails on JDK 21+) and the
Android SDK. Point `local.properties` at your SDK, or set `ANDROID_HOME`.

```bash
cd ScreenBuddy-Android
./gradlew assembleDebug    # debug APK
./gradlew assembleRelease  # minified release APK
```

Release signing reads `ScreenBuddy-Android/keystore.properties` (git-ignored)
or `SCREENBUDDY_KEYSTORE` / `SCREENBUDDY_STORE_PASSWORD` /
`SCREENBUDDY_KEY_PASSWORD`. With no keystore configured, the local release build
falls back to the debug key and logs a warning.

CI does **not** allow that fallback: the `Build signed release APK` job fails if
the key is missing, and verifies the APK is signed by the release key rather
than the debug key. It reads four repository secrets:

| Secret | Value |
|---|---|
| `SCREENBUDDY_KEYSTORE_B64` | `base64 -w0 screenbuddy-release.jks` |
| `SCREENBUDDY_STORE_PASSWORD` | keystore password |
| `SCREENBUDDY_KEY_PASSWORD` | key password |
| `SCREENBUDDY_KEY_ALIAS` | key alias, `screenbuddy` |

Set them with `gh secret set <NAME> --repo LoopyLuci/ScreenBuddy` (pipe the value
on stdin so it stays out of shell history).

> The signing key is **not** recoverable. Android requires every update to a
> published app to be signed by the same key. Keep an offline backup of the
> `.jks` and `keystore.properties` somewhere you control.

### Toolkit (Godot)

The Godot editor is used to author creatures. The engine binaries are **not**
committed — they exceed GitHub's 100 MB per-file limit. Download Godot 4.3 and
either drop it in `toolkit/bin/` or point `GODOT_BIN` at it.

```bash
# List all creatures
./bin/godot-harness.sh --list-creatures

# Validate a creature
./bin/godot-harness.sh --validate companion-bird-01

# Export a creature
./bin/godot-harness.sh --export-creature companion-bird-01 --output ./exports

# Export all creatures
./bin/godot-harness.sh --export-all --output ./exports
```

## Feature parity

The desktop and Android clients cover the same feature set:

| Capability | Desktop (Rust) | Android (Kotlin) |
|---|---|---|
| Multi-provider AI (OpenAI, Anthropic, DeepSeek, Mistral, xAI, OpenRouter, Together, Ollama, self-hosted) | `ai.rs` | `AiService.kt` |
| Agent tool calling | `agent.rs` | `AgentLoop.kt` |
| RAG memory | `rag.rs` (vector store) | `RagPipeline.kt` (BM25, fully offline) |
| Text-to-speech | `tts.rs` | `TtsEngine.kt` |
| Provider/model management | `settings_ui.rs` | `ProvidersScreen.kt` |
| Persistent settings | `config.rs` | `SettingsRepository.kt` |
| Creatures | 7 animated | 7 selectable |
| Encrypted credential storage | config file | Android Keystore (AES-256-GCM) |

Both clients' agent loops dispatch tools, feed results back, and iterate until a
text-only answer or the iteration cap. Neither can get stuck.

## Architecture

```
crates/screenbuddy-core/   # Platform-agnostic core library
  ai.rs                   # Multi-backend AI engine + tool-call parsing
  agent.rs                # Agent runtime: tool dispatch loop
  render/                 # GDI + wgpu rendering backends
  rag.rs                  # Retrieval-augmented memory
crates/screenbuddy/       # Windows desktop app
ScreenBuddy-Android/
  data/repository/        # Room + DataStore persistence
  data/agent/             # Tool-calling loop
  data/rag/               # BM25 retrieval
  service/                # AI backends, TTS
  ui/screens/             # Jetpack Compose UI
toolkit/godot-project/    # Godot creature editor
```

## Testing

```bash
# Desktop: 137 tests
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings

# Android: 77 tests
cd ScreenBuddy-Android && ./gradlew testDebugUnitTest

# Integration suites
python tools/ipc_probe.py           # desktop IPC protocol (app must be running)
python mcp/test_mcp.py              # MCP handshake, discovery, tool calls
python tools/android_control_probe.py --serial <device>   # Android control surface
```

Android unit tests run on the JVM against in-memory fake DAOs, so no emulator is
needed. `ProviderRouteTest` guards the routing table specifically: an unknown
provider must never fall through to another vendor's endpoint, which would leak
the user's credential.

## Using the app

ScreenBuddy opens a main window at startup: sessions on the left, the
conversation on the right, and a message box along the bottom.

## Platform support

Being explicit about what works where, because the boundary is real:

| Component | Windows | Linux | macOS | Android |
| --- | --- | --- | --- | --- |
| Desktop GUI, tray, Direct2D renderer | Yes | No | No | n/a |
| `screenbuddy-core` (AI, RAG, IPC, sessions, hardware) | Yes | Yes | Yes | n/a |
| Android app | n/a | n/a | n/a | Yes |

**The desktop app is Windows-only.** It is Win32/Direct2D throughout and there is
no working implementation for other operating systems. `crates/screenbuddy-core/src/platform/`
contains a `Platform` trait and per-OS stubs, but those are **not implemented** --
they return null handles and do nothing. `create_window` now returns an error
rather than a handle that cannot be used, and `platform::windows_available()`
reports the truth, so nothing mistakes a stub for a window. Real cross-platform
support would mean adopting winit or wgpu surfaces; that has not been done.

The portable core crate is checked on Linux and macOS in CI, which is what
catches a `cfg(windows)` leak or an unguarded x86-only intrinsic.

### Hardware detection

`hardware_profile()` reports the real machine: CPU with architecture and AVX2 /
AVX512 / NEON support, GPUs via wgpu, memory, NUMA node count and OS details.
Two deliberate honesty rules apply:

- **Unmeasured values are reported as unknown, not estimated.** wgpu exposes no
  VRAM figure, so `vram_mb` is 0 with `vram_known: false`. CPU cache sizes are 0
  because the current `sysinfo` version has no accessor. The previous code
  returned a plausible-looking 32/256/8192 KB that was wrong on every machine.
- **CPU architecture is the architecture, not the OS.** It used to report
  `"windows"`.

ISA detection is gated per architecture, so the core builds on aarch64; an
unguarded `is_x86_feature_detected!` does not compile there.

### Agents

Open **Agents** from the tray menu. An *agent* bundles everything that makes one
companion feel like itself: the model that answers, the character on screen, and
how it behaves and speaks.

The editor has three tabs rather than one long form, because a single scrolling
list of twenty fields is how settings screens become unusable:

- **Identity** -- name, character, persona, and a role/instructions box. The
  instructions box is the escape hatch: anything not covered by the fields above
  goes there.
- **Model** -- provider and model, plus temperature, tool steps and timeout.
  Leaving the model on *Automatic* uses whatever your settings already provide.
- **Behaviour** -- movement, whether tools are enabled, and one-click presets.

Five built-in presets ship with the app: **Assistant**, **Coder**, **Study Buddy**,
**Quick Ask** and **Companion**. Presets are read-only, so **Duplicate** makes an
editable copy rather than changing the default for everyone. Your agents persist to
`%APPDATA%\ScreenBuddy\agents.json`.

**Use this agent** in the footer applies a profile: its persona, model,
temperature, tool budget and timeout become the settings the next request uses,
and the choice is restored the next time ScreenBuddy starts. Activation is
separate from saving on purpose, so you can try a change before committing it.

From Hermes:

    screenbuddy_list_agents()                 # includes the presets
    screenbuddy_save_agent({...})             # partial profiles are fine
    screenbuddy_duplicate_agent("builtin-coder")
    screenbuddy_activate_agent("builtin-coder")
    screenbuddy_active_agent()                # what is in effect right now
    screenbuddy_delete_agent("my-agent")

Values are validated and clamped on the way in, so `max_iterations: 0` becomes 1
and `temperature: 99` becomes 2.0 rather than producing an agent that never
answers.

### 9Router

[9Router](https://github.com/decolua/9router) is an OpenAI-compatible router that
fronts many providers with automatic fallback. ScreenBuddy supports it as a
first-class backend rather than as a generic custom endpoint.

Set `nine_router_endpoint` in the AI config (it defaults to
`http://localhost:20128/v1`, 9Router's documented port) and pick a model with
9Router's `provider/model` naming, e.g. `cc/claude-opus-4-6`. `nine_router_api_key`
is optional, since 9Router may run with or without auth.

Model discovery goes through 9Router's `/v1/models`, so the list reflects the
providers you have actually configured rather than a hardcoded set. From Hermes:

    screenbuddy_nine_router_status()            # uses the configured endpoint
    screenbuddy_nine_router_status("http://localhost:20128/v1")

The same request is available over IPC as `nine_router_status`, and the
discovered list is published under the `nine_router.models` setting.

### Tray and window

The app keeps a tray icon in the notification area. Left-click it (or choose
**Open ScreenBuddy**) to bring the window back.

- **Minimise** and **close** both hide the window to the tray rather than
  quitting. A desktop companion that vanished when its window closed looked like
  a crash.
- The tray menu carries Open, Minimise, show/hide creature, open chat, next
  creature, settings, About, and Quit.
- **Quit ScreenBuddy** in the tray menu is the way to actually exit; it also
  removes the icon, so no ghost is left behind.

Minimize-to-tray is only offered when the icon really registered, so the window
can never be hidden with no way back to it.

- **Enter** sends the message; **Backspace** edits it; **Esc** hides the window.
- The sidebar lists every session with its message count and an updated title
  derived from the first thing you asked. Click a session to switch to it, or
  use `+ New chat` and the per-row `-` to delete.
- **Clear conversation** empties the current session without deleting it.
- Sessions persist to `%APPDATA%\ScreenBuddy\sessions.json`, so closing the app
  does not lose the conversation.

The window re-reads the session store every frame, so messages added by the AI
or by an agent over the control API appear without a refresh.

### Settings

Six tabs, and every setting is either live, read-only, or visibly disabled. The
distinction is not cosmetic: a control that looks editable and does nothing is
worse than one that admits it.

- **Live** (10): `animation_speed`, `fps_target`, `rag_enabled`, `volume_master`,
  `volume_effects`, `volume_music`, `ai_provider`, `ai_model`,
  `window_transparency`, `always_on_top`.
- **Set by the app - read only** (3): `collision_avoidance`, `creature_count`,
  `cursor_interaction`. These mirror real runtime state, so they show the truth
  but are not editable.
- **Not wired up yet** (2): `multi_gpu`, `thread_pool_size`. Both say why in the
  source, and both are honest:
  - The desktop app renders through GDI. `MultiGpuRenderer` enumerates real wgpu
    adapters but is never constructed and no code path creates a device, so there
    is no second GPU for the setting to select.
  - `ThreadPool::new()` takes no size and the render loop is single-threaded, so
    a worker count has nothing to configure.

  Wiring either properly means writing a wgpu renderer or parallelising the loop,
  not flipping a flag.

`fps_target` is read per frame and clamped to 5..120; `rag_enabled` gates both
memory ingest and search; `window_transparency` sets per-window alpha with a
floor so a creature cannot be made invisible; `always_on_top` is applied at
window creation and re-applied live via `SetWindowPos`; the two volume settings
drive separate audio buses, with each sound played through the bus for its
category.

Settings persist on every change, with no save button. Out-of-range values are
refused with a reason and the previous value is kept.

Three CI checks keep this honest, and each has caught a real drift:

| Check | Catches |
| --- | --- |
| `dead_settings_scan.py` | a setting the window lists as inert but the app honours, or the reverse |
| `mcp_ipc_audit.py` | a tool or IPC command that is declared but unreachable |
| `wired_settings_probe.py` | a setting that reads back but never reaches runtime state |

## Controlling ScreenBuddy from an agent

The running app exposes a control port, and `mcp/server.py` bridges it to the
Model Context Protocol so an agent can drive it directly:

```bash
cargo build --release   # once
hermes mcp add screenbuddy --command python --args "<repo>/mcp/server.py"
```

The server starts the app itself if it is not already running, so no separate
launch step is needed.

That registers 18 tools (`mcp_screenbuddy_*`) for status, chat, agent runs,
creature movement and animation, audio, settings, and RAG memory. See
[docs/control-api.md](docs/control-api.md) for the full list, the wire protocol,
and the pinning / auto-cycle caveats.

## Releases

Prebuilt binaries are attached to
[GitHub Releases](https://github.com/LoopyLuci/ScreenBuddy/releases):

- `ScreenBuddy-v<version>-windows-x64.exe` — desktop app
- `ScreenBuddy-v<version>-android.apk` — Android app

Stage them locally with `make stage-release VERSION=0.1.0`.

## Creature Format

Creatures are defined as JSON files with the following structure:

```json
{
  "id": "unique-id",
  "name": "Display Name",
  "category": "animal|humanoid|robot|creature|abstract",
  "visual": {
    "animation_format": "sprite_sheet|spine|live2d|frame",
    "texture": "path/to/texture.png",
    "default_scale": 1.0,
    "idle_animation": "idle",
    "action_animations": {
      "thinking": "thinking",
      "success": "celebrate"
    }
  },
  "behavior": {
    "default_state": "idle",
    "autonomous_actions": true,
    "personality": {
      "curiosity": 0.8,
      "energy": 0.7
    }
  },
  "ai": {
    "system_prompt": "You are...",
    "specializations": ["coding", "writing"]
  },
  "permissions": {
    "can_read_screen": true,
    "can_access_files": false
  }
}
```

## F-Droid

The Android client is prepared for F-Droid: no proprietary SDKs (no Play
Services, no Firebase, no tracking), no prebuilt binaries, and it builds from
published source. Store metadata lives in
`ScreenBuddy-Android/fastlane/metadata/android/en-US/`, and the build definition
is `fdroid.yml` in the same directory.

`fdroid.yml` builds the tagged release, guards against a proprietary dependency
being added, and signs with a throwaway key since F-Droid applies its own
signing. Update `Tags:` when cutting a new tag.

Note that F-Droid builds with a FLOSS toolchain, so it cannot use Oracle's JDK.

## License

MIT OR Apache-2.0 — see [LICENSE-MIT](LICENSE-MIT) and
[LICENSE-APACHE](LICENSE-APACHE).
