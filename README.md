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
`SCREENBUDDY_KEY_PASSWORD`. With no keystore configured, the release build falls
back to the debug key and logs a warning.

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
```

Android unit tests run on the JVM against in-memory fake DAOs, so no emulator is
needed. `ProviderRouteTest` guards the routing table specifically: an unknown
provider must never fall through to another vendor's endpoint, which would leak
the user's credential.

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

## License

MIT OR Apache-2.0
