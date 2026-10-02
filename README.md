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

## Architecture

```
crates/screenbuddy-core/   # Platform-agnostic core library
  ai.rs                   # Multi-backend AI engine + tool-call parsing
  agent.rs                # Agent runtime: tool dispatch loop
  render/                 # GDI + wgpu rendering backends
  rag.rs                  # Retrieval-augmented memory
crates/screenbuddy/       # Windows desktop app
ScreenBuddy-Android/      # Native Android app (Kotlin + Compose)
toolkit/godot-project/    # Godot creature editor
```

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
