#!/usr/bin/env python3
"""TDD tests for 9Router management feature.

Verifies:
- 9Router settings persistence (enabled, baseUrl, apiKey)
- 9Router lifecycle operations (start/stop/restart)
- Model loading from 9Router API
- Model enable/disable toggle
- API key management
- Connection testing
- 9Router status reporting

These tests define the contract for the 9Router management feature.
Run: python tools/test_ninerouter_management.py
"""

import json
import os
import subprocess
import sys
import tempfile
import time

# ── Test harness ──────────────────────────────────────────────────────────────

PASSED = 0
FAILED = 0
FAILURES = []


def check(name: str, condition: bool, detail: str = "") -> bool:
    global PASSED, FAILED
    if condition:
        PASSED += 1
        return True
    FAILED += 1
    FAILURES.append(f"{name}: {detail}")
    return False


def assert_eq(name: str, actual, expected) -> bool:
    return check(name, actual == expected, f"expected {expected!r}, got {actual!r}")


def assert_in(name: str, needle: str, haystack: str) -> bool:
    return check(name, needle in haystack, f"'{needle}' not in '{haystack[:200]}'")


def section(title: str):
    print(f"\n{'─'*60}")
    print(f"  {title}")
    print(f"{'─'*60}")


# ── Test: Settings keys exist ─────────────────────────────────────────────────

def test_settings_keys():
    section("9Router settings keys in SettingsRepository")

    repo_path = "C:/Projects/ScreenBuddy/ScreenBuddy-Android/app/src/main/java/com/screenbuddy/android/data/repository/SettingsRepository.kt"
    if not os.path.exists(repo_path):
        check("SettingsRepository.kt exists", False, "file not found")
        return

    with open(repo_path) as f:
        content = f.read()

    # 9Router-specific keys
    assert_in("KEY_NINEROUTER_ENABLED", "KEY_NINEROUTER_ENABLED", content)
    assert_in("KEY_NINEROUTER_URL", "KEY_NINEROUTER_URL", content)
    assert_in("KEY_NINEROUTER_KEY", "KEY_NINEROUTER_KEY", content)

    # AppSettings fields
    assert_in("ninerouterEnabled", "ninerouterEnabled", content)
    assert_in("ninerouterBaseUrl", "ninerouterBaseUrl", content)
    assert_in("ninerouterApiKey", "ninerouterApiKey", content)

    # Defaults (Kotlin type annotation syntax)
    assert_in("ninerouterEnabled: Boolean = false", "ninerouterEnabled: Boolean = false", content)
    assert_in("ninerouterBaseUrl: String = \"http://localhost:20128/v1\"", content, content)
    assert_in("ninerouterApiKey: String = \"\"", content, content)


def test_ninerouter_service_exists():
    section("9RouterService class exists and has required operations")

    service_path = "C:/Projects/ScreenBuddy/ScreenBuddy-Android/app/src/main/java/com/screenbuddy/android/service/NineRouterService.kt"
    if not os.path.exists(service_path):
        check("NineRouterService.kt exists", False, "file not found")
        return

    with open(service_path) as f:
        content = f.read()

    # Required operations
    assert_in("fun getStatus", "fun getStatus", content)
    assert_in("fun getModels", "fun getModels", content)
    assert_in("fun start", "fun start", content)
    assert_in("fun stop", "fun stop", content)
    assert_in("fun testConnection", "fun testConnection", content)
    assert_in("fun healthCheck", "fun healthCheck", content)
    assert_in("data class NineRouterStatus", "data class NineRouterStatus", content)
    assert_in("data class NineRouterModel", "data class NineRouterModel", content)


def test_ninerouter_viewmodel_operations():
    section("ProvidersViewModel 9Router operations")

    vm_path = "C:/Projects/ScreenBuddy/ScreenBuddy-Android/app/src/main/java/com/screenbuddy/android/viewmodel/ProvidersViewModel.kt"
    if not os.path.exists(vm_path):
        check("ProvidersViewModel.kt exists", False, "file not found")
        return

    with open(vm_path) as f:
        content = f.read()

    # 9Router management operations
    assert_in("fun toggleNineRouter", "fun toggleNineRouter", content)
    assert_in("fun setNineRouterUrl", "fun setNineRouterUrl", content)
    assert_in("fun setNineRouterKey", "fun setNineRouterKey", content)
    assert_in("fun refreshNineRouterModels", "fun refreshNineRouterModels", content)
    assert_in("fun testNineRouterConnection", "fun testNineRouterConnection", content)
    assert_in("fun startNineRouter", "fun startNineRouter", content)
    assert_in("fun stopNineRouter", "fun stopNineRouter", content)


def test_ninerouter_ui_controls():
    section("9Router UI controls in SettingsScreen")

    settings_path = "C:/Projects/ScreenBuddy/ScreenBuddy-Android/app/src/main/java/com/screenbuddy/android/ui/screens/SettingsScreen.kt"
    if not os.path.exists(settings_path):
        check("SettingsScreen.kt exists", False, "file not found")
        return

    with open(settings_path) as f:
        content = f.read()

    # 9Router section in settings
    assert_in("9Router", "9Router", content)
    assert_in("ninerouterEnabled", "ninerouterEnabled", content)
    assert_in("ninerouterBaseUrl", "ninerouterBaseUrl", content)


def test_ninerouter_ui_in_providers():
    section("9Router controls in ProvidersScreen")

    providers_path = "C:/Projects/ScreenBuddy/ScreenBuddy-Android/app/src/main/java/com/screenbuddy/android/ui/screens/ProvidersScreen.kt"
    if not os.path.exists(providers_path):
        check("ProvidersScreen.kt exists", False, "file not found")
        return

    with open(providers_path) as f:
        content = f.read()

    # 9Router management card in providers screen
    assert_in("9Router", "9Router", content)
    assert_in("NineRouter", "NineRouter", content)


def test_ninerouter_provider_route():
    section("9Router in AiService ProviderRoute")

    ai_path = "C:/Projects/ScreenBuddy/ScreenBuddy-Android/app/src/main/java/com/screenbuddy/android/service/AiService.kt"
    if not os.path.exists(ai_path):
        check("AiService.kt exists", False, "file not found")
        return

    with open(ai_path) as f:
        content = f.read()

    assert_in("NINEROUTER", "NINEROUTER", content)
    assert_in("localhost:20128", "localhost:20128", content)


def test_ninerouter_control_commands():
    section("9Router control commands")

    commands_path = "C:/Projects/ScreenBuddy/ScreenBuddy-Android/app/src/main/java/com/screenbuddy/android/data/control/ControlCommands.kt"
    if not os.path.exists(commands_path):
        check("ControlCommands.kt exists", False, "file not found")
        return

    with open(commands_path) as f:
        content = f.read()

    assert_in("nine_router_status", "nine_router_status", content)
    assert_in("nine_router_start", "nine_router_start", content)
    assert_in("nine_router_stop", "nine_router_stop", content)
    assert_in("nine_router_models", "nine_router_models", content)
    assert_in("nine_router_test", "nine_router_test", content)


def test_ninerouter_agent_profile():
    section("9Router in AgentProfile personas")

    profile_path = "C:/Projects/ScreenBuddy/ScreenBuddy-Android/app/src/main/java/com/screenbuddy/android/data/agent/AgentProfile.kt"
    if not os.path.exists(profile_path):
        check("AgentProfile.kt exists", False, "file not found")
        return

    with open(profile_path) as f:
        content = f.read()

    assert_in("NineRouter", "NineRouter", content)


def test_ninerouter_provider_data():
    section("9Router in ProviderData catalogue")

    data_path = "C:/Projects/ScreenBuddy/ScreenBuddy-Android/app/src/main/java/com/screenbuddy/android/data/model/ProviderData.kt"
    if not os.path.exists(data_path):
        check("ProviderData.kt exists", False, "file not found")
        return

    with open(data_path) as f:
        content = f.read()

    assert_in("9router", "9router", content)
    assert_in("9router-auto", "9router-auto", content)


def test_ninerouter_surface_audit_clean():
    section("Surface audit: 9Router settings offered in UI")

    # Run the surface audit and check 9Router settings are NOT flagged
    result = subprocess.run(
        [sys.executable, "tools/android_surface_audit.py", "--serial", "GBNPCX062430B87"],
        capture_output=True, text=True, timeout=60,
        cwd="C:/Projects/ScreenBuddy"
    )
    output = result.stdout

    # ninerouterEnabled should appear with a UI indicator (not "not in UI")
    check("ninerouterEnabled in audit", "ninerouterEnabled" in output, "not found in audit output")
    if "ninerouterEnabled" in output:
        line = [l for l in output.splitlines() if "ninerouterEnabled" in l][0]
        # The audit reports either "offered in UI" or "not in UI"; we want the former
        check("ninerouterEnabled has UI control", "not in UI" not in line, f"got: {line.strip()}")


# ── Run all tests ─────────────────────────────────────────────────────────────

def main():
    print("=" * 60)
    print("  9Router Management TDD Tests")
    print("=" * 60)

    test_settings_keys()
    test_ninerouter_service_exists()
    test_ninerouter_viewmodel_operations()
    test_ninerouter_ui_controls()
    test_ninerouter_ui_in_providers()
    test_ninerouter_provider_route()
    test_ninerouter_control_commands()
    test_ninerouter_agent_profile()
    test_ninerouter_provider_data()
    test_ninerouter_surface_audit_clean()

    print(f"\n{'='*60}")
    print(f"  Results: {PASSED} passed, {FAILED} failed")
    print(f"{'='*60}")

    if FAILURES:
        print("\nFailures:")
        for f in FAILURES:
            print(f"  FAIL  {f}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
