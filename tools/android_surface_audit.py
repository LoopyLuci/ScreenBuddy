"""Audit the Android app the way dead_settings_scan.py audits the desktop.

Four defects on the desktop had the same shape: a setting or control that is
declared, offered in the UI, and then never read. Every one was found by
scanning for real consumers rather than by reading the code.

Android's AppSettings declares 17 fields and the settings screen exposes most of
them. This finds which are actually read by a consumer - a Compose control that
writes a value nothing reads looks exactly like a working one.
"""

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
ANDROID = ROOT / "ScreenBuddy-Android" / "app" / "src"
MAIN = ANDROID / "main" / "java"
COM = MAIN / "com" / "screenbuddy" / "android"

REPO = COM / "data" / "repository" / "SettingsRepository.kt"

# The declared fields, and the DataStore key each maps to.
SETTER_KEY = {
    "darkMode": "KEY_DARK_MODE",
    "notificationsEnabled": "KEY_NOTIFICATIONS",
    "soundEffectsEnabled": "KEY_SOUND_EFFECTS",
    "autoStart": "KEY_AUTO_START",
    "streamResponses": "KEY_STREAM",
    "showCreatures": "KEY_SHOW_CREATURES",
    "animationsEnabled": "KEY_ANIMATIONS",
    "creatureSoundsEnabled": "KEY_CREATURE_SOUNDS",
    "ttsEnabled": "KEY_TTS",
    "masterVolume": "KEY_MASTER_VOLUME",
    "effectsVolume": "KEY_EFFECTS_VOLUME",
    "ttsVolume": "KEY_TTS_VOLUME",
    "animationSpeed": "KEY_ANIMATION_SPEED",
    "temperature": "KEY_TEMPERATURE",
    "responseLength": "KEY_RESPONSE_LENGTH",
    "selectedModelId": "KEY_SELECTED_MODEL",
    "ollamaBaseUrl": "KEY_OLLAMA_URL",
}


def kotlin_files():
    return sorted(p for p in MAIN.rglob("*.kt"))


def declared_fields():
    """Names declared in the AppSettings data class."""
    text = REPO.read_text(encoding="utf-8")
    m = re.search(r"data class AppSettings\s*\((.*?)\n\)", text, re.S)
    if not m:
        raise SystemExit("could not find the AppSettings data class")
    return re.findall(r"^\s*val (\w+):", m.group(1), re.M)


# Files that only move a value between the UI and the repository. Reading a
# field here is storage, not behaviour: SettingsViewModel mirrors what the screen
# wrote, so counting it made every setting look live.
PLUMBING = (
    "data/repository/SettingsRepository.kt",
    "viewmodel/SettingsViewModel.kt",
    "ui/screens/SettingsScreen.kt",
)


def read_sites(field):
    """Files outside the settings plumbing that read a field."""
    hits = []
    pattern = re.compile(rf"\b{field}\b")
    for path in kotlin_files():
        rel = path.relative_to(COM).as_posix()
        if path == REPO or rel in PLUMBING:
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        for n, line in enumerate(text.splitlines(), 1):
            if pattern.search(line):
                hits.append(f"{rel}:{n}")
    return hits


def written_by_ui(field):
    """Whether a screen writes this field, i.e. the UI offers a control for it."""
    hits = []
    pattern = re.compile(rf"(?:set|update)\w*{field[0].upper()}{field[1:]}\b")
    for path in kotlin_files():
        if "ui" not in path.parts:
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        rel = path.relative_to(COM).as_posix()
        for n, line in enumerate(text.splitlines(), 1):
            if pattern.search(line):
                hits.append(f"{rel}:{n}")
    return hits


def control_commands():
    """Commands advertised in ControlCommands.SUPPORTED."""
    path = COM / "data" / "control" / "ControlCommands.kt"
    text = path.read_text(encoding="utf-8")
    m = re.search(r"val SUPPORTED = listOf\((.*?)\n    \)", text, re.S)
    if not m:
        raise SystemExit("could not find ControlCommands.SUPPORTED")
    return re.findall(r'"([a-z_]+)"', m.group(1))


def control_registered():
    """Commands actually wired to a handler via ControlBus.register."""
    path = COM / "data" / "control" / "ControlCommands.kt"
    text = path.read_text(encoding="utf-8")
    return set(re.findall(r'ControlBus\.register\(\s*"([a-z_]+)"', text))


def main():
    fields = declared_fields()
    print(f"{len(fields)} settings declared in AppSettings\n")

    live, dead = [], []
    for field in fields:
        reads = read_sites(field)
        offered = written_by_ui(field)
        if reads:
            live.append((field, reads, offered))
        else:
            dead.append((field, offered))

    print(f"READ BY SOMETHING ({len(live)}):")
    for field, reads, offered in live:
        mark = "offered in UI" if offered else "not in UI"
        first = reads[0]
        print(f"  {field:<24} {mark:<14} {first} (+{len(reads)-1} more)")

    print(f"\nREAD BY NOTHING ({len(dead)}):")
    for field, offered in dead:
        note = "but the UI offers a control for it" if offered else "not offered either"
        print(f"  {field:<24} {note}")

    # A command in SUPPORTED but never registered is advertised to agents and
    # then does nothing at runtime: the same defect as an inert setting.
    advertised = control_commands()
    registered = control_registered()
    missing = set(advertised) - registered
    extra = registered - set(advertised)

    print(f"\nCommands advertised ({len(advertised)}), registered ({len(registered)}):")
    print(f"  advertised but never registered ({len(missing)}):")
    for name in sorted(missing):
        print(f"    {name}")
    if not missing:
        print("    (none)")
    print(f"  registered but not advertised ({len(extra)}):")
    for name in sorted(extra):
        print(f"    {name}")
    if not extra:
        print("    (none)")

    print(
        f"\n{len(live)} live, {len(dead)} unconsumed, "
        f"{len(advertised)} commands, {len(missing)} unreachable"
    )
    # Unconsumed settings do not fail the build: each one is disclosed in the UI
    # with the missing capability named, so the finding is already reported. What
    # must fail is a command advertised but unable to run, because an agent would
    # be told it is available and it would do nothing.
    return 0 if not missing and not extra else 1
    # Reporting only: this run documents Android rather than gating a list, since
    # there is no hand-maintained classification to drift yet.
    return 0


if __name__ == "__main__":
    sys.exit(main())