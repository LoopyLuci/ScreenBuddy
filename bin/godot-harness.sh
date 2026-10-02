#!/usr/bin/env bash
# Godot CLI wrapper for ScreenBuddy toolkit
# 
# Usage:
#   ./godot-harness.sh --export-creature <creature_id> --output <dir>
#   ./godot-harness.sh --export-all --output <dir>
#   ./godot-harness.sh --list-creatures
#   ./godot-harness.sh --validate <creature_id>
#   ./godot-harness.sh --headless-editor

set -euo pipefail

GODOT_PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)/toolkit/godot-project"
GODOT_BIN="${GODOT_BIN:-$(find "$PWD/toolkit/bin" -name 'Godot_*_console.exe' 2>/dev/null | head -1)}"
EXPORT_DIR=""

run_godot() {
    if [ -z "$GODOT_BIN" ] || [ ! -f "$GODOT_BIN" ]; then
        echo "ERROR: Godot binary not found. Set GODOT_BIN env var."
        exit 1
    fi
    echo "[harness] Running: $GODOT_BIN $@"
    "$GODOT_BIN" "$@"
}

export_creature() {
    local creature_id="$1"
    local output_dir="$2"
    echo "[harness] Exporting creature: $creature_id -> $output_dir"
    run_godot --headless --path "$GODOT_PROJECT_DIR" --script "res://scripts/harness.gd" "export=$creature_id" "output=$output_dir"
}

export_all() {
    local output_dir="$1"
    echo "[harness] Exporting all creatures -> $output_dir"
    run_godot --headless --path "$GODOT_PROJECT_DIR" --script "res://scripts/harness.gd" "export-all" "output=$output_dir"
}

list_creatures() {
    echo "[harness] Listing available creatures..."
    run_godot --headless --path "$GODOT_PROJECT_DIR" --script "res://scripts/harness.gd" "list"
}

validate_creature() {
    local creature_id="$1"
    echo "[harness] Validating creature: $creature_id"
    run_godot --headless --path "$GODOT_PROJECT_DIR" --script "res://scripts/harness.gd" "validate=$creature_id"
}

case "${1:-}" in
    --export-creature)
        export_creature "$2" "${3:-$PWD/exports}"
        ;;
    --export-all)
        export_all "${2:-$PWD/exports}"
        ;;
    --list-creatures)
        list_creatures
        ;;
    --validate)
        validate_creature "$2"
        ;;
    --headless-editor)
        run_godot --headless --path "$GODOT_PROJECT_DIR"
        ;;
    --help|-h)
        echo "Usage: $0 [OPTION]"
        echo ""
        echo "Options:"
        echo "  --export-creature <id> [output_dir]  Export a single creature"
        echo "  --export-all [output_dir]            Export all creatures"
        echo "  --list-creatures                     List available creatures"
        echo "  --validate <id>                      Validate a creature"
        echo "  --headless-editor                    Run headless editor"
        echo "  --help                               Show this help"
        ;;
    *)
        echo "Unknown option: ${1:-}"
        echo "Use --help for usage information."
        exit 1
        ;;
esac
