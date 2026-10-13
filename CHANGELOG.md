# Changelog

All notable changes to ScreenBuddy are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.6.2] - 2026-10-13

Wire the dpi, accessibility, and eventlog modules into the running app and gate the declared-vs-consumed audit. Per-monitor V2 DPI awareness, pet windows clamped to their display, screen-reader notifications on window show, high-contrast theme auto-applied from the OS, and startup/shutdown recorded in the Windows Application log. Fixes 10 pre-existing clippy errors surfaced by --all-targets.

## [0.6.2] - 2026-10-13

Wire the dpi, accessibility, and eventlog modules into the running app and gate the declared-vs-consumed audit. Per-monitor V2 DPI awareness, pet windows clamped to their display, screen-reader notifications on window show, high-contrast theme auto-applied from the OS, and startup/shutdown recorded in the Windows Application log. Fixes 10 pre-existing clippy errors surfaced by --all-targets.

## [0.6.1] - 2026-10-02

### Added
- Dynamic tray menu with always-on-top toggle and conditional Next creature
- Performance benchmarking mode (`--benchmark` CLI flag)
- Crash reporting via Windows minidumps
- Auto-update checking with version comparison
- Multi-monitor DPI awareness
- Long-running stability soak test mode (`--soak` CLI flag)
- Windows Event Log integration for critical operations
- User documentation: keyboard shortcuts, settings reference, tray guide

### Changed
- Removed wgpu dependency; use Win32 EnumDisplayDevicesW for GPU detection
- Fixed frame pacing: every frame cost twice its budget
- Fixed fps_target startup bug
- Settings profiles: export and import over IPC and from the window
- Keyboard-only navigation: Tab between regions, arrows in the session list
- Play a sound when a message is sent and when a reply arrives

### Fixed
- Test colour constants to match theme system
- All clippy warnings removed
