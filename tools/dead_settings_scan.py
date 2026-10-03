"""Find settings that nothing reads.

Three defects in a row had the same shape: something that looked finished, and
nothing on the other end. This scans every declared setting and reports whether
any code actually reads it, so the gap is measured rather than guessed at.
"""

import pathlib
import re
import subprocess

ROOT = pathlib.Path(__file__).resolve().parent.parent
CORE = ROOT / "crates" / "screenbuddy-core" / "src"
BIN = ROOT / "crates" / "screenbuddy" / "src"


def declared_settings():
    """Names registered in SettingsUI, with their category."""
    text = (CORE / "settings_ui.rs").read_text(encoding="utf-8")
    out = []
    for block in re.findall(r"SettingDef \{.*?\}", text, re.S):
        name = re.search(r'name: "([a-z_]+)"', block)
        cat = re.search(r"category: SettingsCategory::(\w+)", block)
        if name:
            out.append((name.group(1), cat.group(1) if cat else "?"))
    return out


def app_writes(name):
    """Whether the app pushes a value into this setting, rather than reading it.

    `settings.set(...)` at startup means the setting mirrors real state: the
    window shows the truth, but changing it does nothing. That is a third state,
    distinct from both honoured and inert.
    """
    # Look for the name inside a set() call specifically: a name merely
    # appearing near an unrelated .set() would wrongly mark it as mirrored.
    for path in BIN.glob("*.rs"):
        text = path.read_text(encoding="utf-8", errors="replace")
        for match in re.finditer(r"\.set\(", text):
            # Take the surrounding call expression, up to the closing paren.
            start = text.rfind("\n", 0, match.start())
            window = text[max(0, start - 200) : match.end() + 200]
            if f'"{name}"' in window:
                return True
    return False


def consumers(name):
    """Where, if anywhere, the app reads this setting."""
    hits = []
    for path in BIN.glob("*.rs"):
        text = path.read_text(encoding="utf-8", errors="replace")
        for i, line in enumerate(text.splitlines(), 1):
            if name in line and "get(" in line:
                hits.append(f"{path.name}:{i}")
            elif f'"{name}"' in line and ("number(" in line or ".get(" in line):
                hits.append(f"{path.name}:{i}")
    return hits


def main():
    settings = declared_settings()
    print(f"{len(settings)} settings declared\n")
    dead, mirrored, live = [], [], []
    for name, category in sorted(settings):
        where = consumers(name)
        if where:
            live.append((name, category, where))
        elif app_writes(name):
            mirrored.append((name, category))
        else:
            dead.append((name, category))

    print(f"READ BY SOMETHING ({len(live)}):")
    for name, category, where in live:
        print(f"  {name:<24} {category:<12} {', '.join(where[:2])}")
    print(f"\nWRITTEN BUT NEVER READ ({len(mirrored)}):")
    for name, category in mirrored:
        print(f"  {name:<24} {category}")
    print(f"\nREAD BY NOTHING ({len(dead)}):")
    for name, category in dead:
        print(f"  {name:<24} {category}")

    # Compare with the window's own list, which should agree.
    win = (BIN / "settings_window.rs").read_text(encoding="utf-8")
    block = re.search(r"INERT_SETTINGS: &\[&str\] = &\[(.*?)\];", win, re.S)
    listed = set(re.findall(r'"([a-z_]+)"', block.group(1))) if block else set()

    mirror_block = re.search(r"MIRRORED_SETTINGS: &\[&str\] = &\[(.*?)\];", win, re.S)
    mirrored_listed = (
        set(re.findall(r'"([a-z_]+)"', mirror_block.group(1))) if mirror_block else set()
    )
    actual = {n for n, _ in dead}
    actual_mirrored = {n for n, _ in mirrored}

    print()
    ok = True
    if listed == actual:
        print("PASS  the window's inert list matches what is actually dead")
    else:
        print("FAIL  the window's inert list is wrong")
        print(f"  listed but not dead: {sorted(listed - actual)}")
        print(f"  dead but unlisted: {sorted(actual - listed)}")
        ok = False

    if mirrored_listed == actual_mirrored:
        print("PASS  the window's read-only list matches what the app only writes")
    else:
        print("FAIL  the window's read-only list is wrong")
        print(f"  listed but not mirrored: {sorted(mirrored_listed - actual_mirrored)}")
        print(f"  mirrored but unlisted: {sorted(actual_mirrored - mirrored_listed)}")
        ok = False

    if listed & mirrored_listed:
        print(f"FAIL  overlap between the two lists: {sorted(listed & mirrored_listed)}")
        ok = False

    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())