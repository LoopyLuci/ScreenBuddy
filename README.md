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

### Godot Editor (Creature Creation)

1. Open `toolkit/godot-project/project.godot` in Godot 4.3
2. Use the creature editor to create and modify creatures
3. Export creatures to `exports/` directory

### Godot CLI (Headless)

```bash
# List all creatures
./toolkit/bin/godot-harness.sh --list-creatures

# Validate a creature
./toolkit/bin/godot-harness.sh --validate companion-bird-01

# Export a creature
./toolkit/bin/godot-harness.sh --export-creature companion-bird-01 --output ./exports

# Export all creatures
./toolkit/bin/godot-harness.sh --export-all --output ./exports
```

### Rust Core

```bash
# Build the core library
cargo build --package screenbuddy-core

# Run tests
cargo test --package screenbuddy-core
```

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
