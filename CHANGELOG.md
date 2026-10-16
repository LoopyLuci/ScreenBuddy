# Changelog

All notable changes to ScreenBuddy are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.8.0] - 2026-10-15

The fleet becomes a swarm, the creature's tools get sandboxed, and the phone becomes a remote

The four remaining integration patterns from the master plan ship in one release, each verified
against a running fleet:

- The swarm (M): N ScreenBuddy instances on distinct IPC ports, each its own backend kind. A
  `fanout_synthesize` run answered on three creatures at once, and each answer celebrated on
  the creature that produced it — proven by an A/B test with auto-cycle disabled.
- The sandbox (L): the creature's dangerous tools (mutation verbs, shell, web, fetch) no longer
  take the direct MCP path. The app routes them through a new
  `POST /api/screenbuddy/sandbox_exec`, which runs the tool in a bounded process_cell with CPU,
  memory and wall-clock limits; a timeout kills the whole tree and says so.
- The fine-tune seam (N): `POST /api/screenbuddy/finetune` turns the user model's approved
  items into a real JSONL training dataset, and trains a LoRA where torch+CUDA+unsloth exist —
  reporting the exact missing piece where they don't.
- The phone as a remote (F): a new Fleet tab on Android drives the same app-agnostic extension
  routes — creature status, the router's recommendation, the fine-tune pipeline — under the
  scoped integration key, so the phone and the desktop share one command surface.

## [0.7.0] - 2026-10-15

The AgenticBotPlatform integration, hardened and shipped

Desktop and fleet now ship the AgenticBotPlatform integration as a
first-class citizen, every layer of it verified against a live fleet:

- The MCP bridge survives the fleet restarting: it reconnects with
  backoff, self-heals a poisoned mutex, and a stale connection is
  replaced instead of answered through a dead socket.
- The agent's tool surface is configurable: ABP_TOOLS takes an allowlist
  and ABP_TINY_TOOLS selects a curated read-only set, so a 0.5B local
  model gets 11 clean schemas instead of 72 mutation verbs.
- Asks to the fleet are reliable: correlation IDs on every payload and
  error, one retry with backoff for transient failures only (never a 4xx
  or ok:false), a hard timeout, and the task class sent as a hint.
- On the fleet side, the screenbuddy extension (zero core edits, add_*
  hooks only) adds: a scoped integration key limited to its own routes;
  outcome and recommend routes that close the learning loop into the
  router brain; the creature's memory as a fleet memory source; an event
  watcher that posts fleet and creature status to the shared board and
  celebrates terminal jobs only; persona continuity, so an instance and
  a creature agent stay bound across restarts; a Creature panel on the
  dashboard; and the creature's exchanges folded into the user model.

## [0.6.3] - 2026-10-14

Fix a startup panic and a 404 on every AI request

The v0.6.2 binary crashed on launch before opening its control port (the
auto-update check called tokio::spawn from main with no runtime) and, once
running, returned a canned fallback instead of a model reply: the default
pinned model 'llama3.2' was a catalogue name the user never pulled, so every
chat request 404'd. Both are fixed and verified by the live-model CI job,
which now completes a real conversation against Ollama.

Also: gate Windows-only code and metric assertions behind cfg(windows) so the
core crate builds on Linux and macOS; wire the always_on_top and
window_transparency settings that were declared but never read; advertise the
five nine_router commands in the Android SUPPORTED list; and make the
live-model check print emoji-containing replies as UTF-8 instead of crashing
on a cp1252 console.

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
