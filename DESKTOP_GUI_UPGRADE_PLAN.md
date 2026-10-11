# ScreenBuddy Desktop GUI — Complete Execution Plan

## Overview

15 critical gaps across 6 subsystems, organized into 4 phases with strict dependency ordering.
Each task specifies: files changed, exact changes, test strategy, and rollback.

---

## Phase 1 — Fix False Surfaces (Dead Controls)

**Goal:** Every control the user sees must do something real. No stubs, no dead code, no false affordances.

### Task 1.1: Remove dead `GdiRenderer` code
- **Files:** `crates/screenbuddy/src/gdi_renderer.rs` (delete), `crates/screenbuddy/src/main.rs` (remove `mod` declaration)
- **Changes:** Delete `gdi_renderer.rs`. Remove `mod gdi_renderer;` from `main.rs`. Verify no references exist.
- **Test:** `cargo build --release` succeeds. `cargo test --workspace` passes.
- **Rollback:** `git revert` the commit.

### Task 1.2: Remove empty per-pet windows
- **Files:** `crates/screenbuddy/src/main.rs` (lines ~1698-1745, ~1747-1769)
- **Changes:** Remove `PerPetWindowManager` creation, `pet_window_proc`, per-pet window creation loop, and the two `pet_window_message_loop()` calls. Keep the composite renderer (it works).
- **Test:** `cargo build --release` succeeds. App starts, creatures render in composite window. `ipc_probe.py` passes 11/11.
- **Rollback:** `git revert`.

### Task 1.3: Fix `ai_model` string cycling stub
- **Files:** `crates/screenbuddy/src/settings_window.rs` (lines ~893-898)
- **Changes:** Replace the "true"↔"false" cycling with a real dropdown or text input. Show the current model name. On click, cycle through available models from the AI service. If no models available, show "No models configured".
- **Test:** `cargo test -p screenbuddy --lib` passes. Manual: click ai_model field, see model name change.
- **Rollback:** `git revert`.

### Task 1.4: Populate `chat_history` in `publish_status()`
- **Files:** `crates/screenbuddy/src/main.rs` (lines ~1124-1132)
- **Changes:** Add `"chat_history" to snapshot.chat_history` to the status map. Ensure `chat_history` is kept up to date when messages are sent/received.
- **Test:** `ipc_probe.py` passes 11/11. `mcp/test_mcp.py` passes 13/13. `screenbuddy_chat_history` returns messages.
- **Rollback:** `git revert`.

### Task 1.5: Fix `RunAgent` stub
- **Files:** `crates/screenbuddy/src/main.rs` (lines ~1193-1198)
- **Changes:** Wire `RunAgent` to `AgentRuntime`. Parse the prompt from the request, create a chat message, send through the AI service, persist the reply. Return the reply text in the response.
- **Test:** `agent_probe.py` passes 11/11. Send a chat via IPC, verify reply comes back.
- **Rollback:** `git revert`.

### Task 1.6: Apply `animation_speed` to animator
- **Files:** `crates/screenbuddy/src/main.rs` (lines ~858-861)
- **Changes:** Read `animation_speed` from settings each frame. Pass it to the creature animator as a multiplier on frame duration. Cache the last applied value to avoid redundant work.
- **Test:** `settings_probe.py` passes 7/7. Change animation_speed, verify creature animation rate changes.
- **Rollback:** `git revert`.

### Task 1.7: Wire cursor interaction
- **Files:** `crates/screenbuddy/src/main.rs` (line ~400)
- **Changes:** Track `WM_MOUSEMOVE` in the main window proc. Update `cursor_pos` in the render loop. When `cursor_interaction` is enabled, creatures near the cursor should react (look at cursor, move away, etc.).
- **Test:** `ipc_probe.py` passes 11/11. Move mouse over window, verify creature reacts.
- **Rollback:** `git revert`.

### Task 1.8: Wire drag to creature movement
- **Files:** `crates/screenbuddy/src/main.rs` (line ~401)
- **Changes:** When `WM_LBUTTONDOWN` hits a creature, set `dragged_creature`. On `WM_MOUSEMOVE`, move that creature. On `WM_LBUTTONUP`, clear `dragged_creature`.
- **Test:** `ipc_probe.py` passes 11/11. Drag creature with mouse, verify it follows.
- **Rollback:** `git revert`.

### Task 1.9: Remove double message pump
- **Files:** `crates/screenbuddy/src/main.rs` (lines ~830 and ~1056)
- **Changes:** Remove the second `pet_window_message_loop()` call (step 15 in the render loop). Keep the first one (step 7).
- **Test:** `cargo build --release` succeeds. App runs without hangs. `ipc_probe.py` passes 11/11.
- **Rollback:** `git revert`.

### Phase 1 Verification
- `cargo build --release` — success
- `cargo test --workspace` — all pass
- `ipc_probe.py` — 11/11
- `mcp/test_mcp.py` — 13/13
- `settings_probe.py` — 7/7
- `agent_probe.py` — 11/11
- `nine_router_probe.py` — 5/5
- `live_desktop_check.py` — 9/9
- `live_model_check.py` — 7/7

---

## Phase 2 — Core Correctness

**Goal:** The app runs at the right speed, on any display, without falling behind.

### Task 2.1: Implement real delta time
- **Files:** `crates/screenbuddy/src/main.rs` (line ~677)
- **Changes:** Replace `let dt = 1.0 / 30.0;` with `let dt = frame_start.elapsed().as_secs_f64();` where `frame_start = Instant::now()` at the top of the loop. Clamp `dt` to [0, 0.1] to prevent huge jumps after a stall.
- **Test:** `cargo build --release` succeeds. App runs at correct speed. `settings_probe.py` passes 7/7.
- **Rollback:** `git revert`.

### Task 2.2: Add frame skipping
- **Files:** `crates/screenbuddy/src/main.rs` (render loop)
- **Changes:** If `dt > 0.05` (loop running below 20fps), skip the physics update and only render. This prevents slow-motion when the loop falls behind.
- **Test:** `cargo build --release` succeeds. App stays responsive under load.
- **Rollback:** `git revert`.

### Task 2.3: Add DPI awareness
- **Files:** `crates/screenbuddy/src/main.rs` (top of `main()`), all window creation functions
- **Changes:** Call `SetProcessDPIAware()` at the start of `main()`. Handle `WM_DPICHANGED` in each window proc: reposition and resize controls based on the new DPI. Scale all pixel constants by `GetDpiForWindow(hwnd) / 96.0`.
- **Test:** `cargo build --release` succeeds. Window renders correctly at 100%, 125%, 150% DPI.
- **Rollback:** `git revert`.

### Task 2.4: Improve sleep precision
- **Files:** `crates/screenbuddy/src/main.rs` (line ~1130)
- **Changes:** Replace `std::thread::sleep` with a waitable timer or a hybrid: `sleep` for most of the duration, then busy-wait for the remainder. Target < 2ms precision.
- **Test:** `cargo build --release` succeeds. Frame pacing is consistent.
- **Rollback:** `git revert`.

### Phase 2 Verification
- All Phase 1 tests still pass
- App runs at correct speed on high-DPI display
- No slow-motion under load

---

## Phase 3 — User Experience

**Goal:** Every feature a user expects from a production app works correctly.

### Task 3.1: Add scrolling to chat window
- **Files:** `crates/screenbuddy/src/chat_window.rs`
- **Changes:** Add `scroll_offset: usize` to chat state. Handle `WM_MOUSEWHEEL` to adjust offset. Render only visible messages. Add a scrollbar indicator on the right edge. Clamp offset to [0, max(0, total - visible)].
- **Test:** `cargo test -p screenbuddy --lib` passes. Scroll with mouse wheel, verify messages move.
- **Rollback:** `git revert`.

### Task 3.2: Add scrolling to settings window
- **Files:** `crates/screenbuddy/src/settings_window.rs`
- **Changes:** Add `scroll_offset: usize` to settings state. Handle `WM_MOUSEWHEEL`. Render only visible rows. Add scrollbar. Replace the "more settings below" text with actual scrolling.
- **Test:** `cargo test -p screenbuddy --lib` passes. All settings reachable by scrolling.
- **Rollback:** `git revert`.

### Task 3.3: Make instructions field editable
- **Files:** `crates/screenbuddy/src/agent_editor.rs` (lines ~493-496)
- **Changes:** Add `Field::Instructions` to the `on_char` handler. Support Enter for newlines, Backspace, arrow keys for cursor movement. Add cursor rendering in the multiline field. Add scrolling within the field when content exceeds the area.
- **Test:** `cargo test -p screenbuddy --lib` passes. Type instructions, verify text appears.
- **Rollback:** `git revert`.

### Task 3.4: Add multiline chat input
- **Files:** `crates/screenbuddy/src/chat_window.rs`
- **Changes:** Change input field to multiline (3 lines). Enter sends, Shift+Enter inserts newline. Render multiple lines with cursor. Auto-grow up to 5 lines.
- **Test:** `cargo test -p screenbuddy --lib` passes. Shift+Enter inserts newline, Enter sends.
- **Rollback:** `git revert`.

### Task 3.5: Add timestamps to chat messages
- **Files:** `crates/screenbuddy/src/chat_window.rs`
- **Changes:** Store timestamp with each message. Render as "HH:MM" prefix in muted color. Toggle with a setting.
- **Test:** `cargo test -p screenbuddy --lib` passes. Messages show timestamps.
- **Rollback:** `git revert`.

### Task 3.6: Add copy/paste in chat
- **Files:** `crates/screenbuddy/src/chat_window.rs`
- **Changes:** Add Ctrl+C to copy selected message (or last message if no selection). Add right-click context menu with Copy, Select All, Clear.
- **Test:** `cargo test -p screenbuddy --lib` passes. Ctrl+C copies message text.
- **Rollback:** `git revert`.

### Task 3.7: Add typing indicator
- **Files:** `crates/screenbuddy/src/chat_window.rs`
- **Changes:** When a message is sent, show animated "..." indicator. Remove when reply arrives. Animate with a timer (3 frames: ".", "..", "...").
- **Test:** `cargo test -p screenbuddy --lib` passes. Typing indicator appears during AI response.
- **Rollback:** `git revert`.

### Task 3.8: Add send button to chat
- **Files:** `crates/screenbuddy/src/chat_window.rs`
- **Changes:** Add a clickable send button (paper plane icon or "Send" text) to the right of the input field. Disabled when input is empty.
- **Test:** `cargo test -p screenbuddy --lib` passes. Click send button, message sends.
- **Rollback:** `git revert`.

### Task 3.9: Add keyboard navigation in settings
- **Files:** `crates/screenbuddy/src/settings_window.rs`
- **Changes:** Add `focused_control: usize` state. Handle Tab/Shift+Tab to move focus. Handle Up/Down arrows. Handle Space/Enter to activate. Draw focus rectangle on focused control.
- **Test:** `cargo test -p screenbuddy --lib` passes. Tab navigates between controls.
- **Rollback:** `git revert`.

### Task 3.10: Add search/filter in settings
- **Files:** `crates/screenbuddy/src/settings_window.rs`
- **Changes:** Add a search box at the top. Filter settings by name/description as the user types. Show "N matching settings" when filtering. Clear on Escape.
- **Test:** `cargo test -p screenbuddy --lib` passes. Type in search box, settings filter.
- **Rollback:** `git revert`.

### Phase 3 Verification
- All Phase 1 + 2 tests still pass
- Chat: scrolling, multiline, timestamps, copy/paste, typing indicator, send button
- Settings: scrolling, keyboard nav, search
- Agent editor: editable instructions

---

## Phase 4 — Production Polish

**Goal:** The app feels like a premium, production-grade product.

### Task 4.1: Extract theme system
- **Files:** New file `crates/screenbuddy/src/theme.rs`, all UI files
- **Changes:** Create `Theme` struct with all colors, fonts, spacing. Implement `DarkTheme` and `LightTheme`. Replace all hardcoded colors with `theme.color(...)`. Add a `theme` setting to switch between dark/light.
- **Test:** `cargo test --workspace` passes. Theme switches correctly.
- **Rollback:** `git revert`.

### Task 4.2: Add IPC authentication
- **Files:** `crates/screenbuddy-core/src/ipc.rs`, `crates/screenbuddy/src/main.rs`
- **Changes:** Generate a random token at startup. Require `Authorization: Bearer <token>` header on IPC connections. Print the token to stdout and a file. Reject unauthenticated commands.
- **Test:** `ipc_probe.py` passes 11/11 (updated to include token). Unauthenticated connection rejected.
- **Rollback:** `git revert`.

### Task 4.3: Add custom tray icon
- **Files:** `crates/screenbuddy-core/src/system_tray.rs`, `crates/screenbuddy/src/main.rs`
- **Changes:** Embed an icon resource in the executable. Load it with `LoadImageW`. Use it for `NOTIFYICONDATAW.hIcon`. Fallback to `IDI_APPLICATION` if loading fails.
- **Test:** `cargo test --workspace` passes. Tray icon shows custom icon.
- **Rollback:** `git revert`.

### Task 4.4: Add balloon notifications
- **Files:** `crates/screenbuddy-core/src/system_tray.rs`
- **Changes:** Add `NIF_INFO` flag to `NOTIFYICONDATAW`. Implement `show_balloon(title, message)` using `Shell_NotifyIconW(NIM_MODIFY, ...)`. Call it on AI reply, agent completion, errors.
- **Test:** `cargo test --workspace` passes. Balloon appears on AI reply.
- **Rollback:** `git revert`.

### Task 4.5: Add dynamic tray menu
- **Files:** `crates/screenbuddy-core/src/system_tray.rs`
- **Changes:** Rebuild menu items each time based on current state. Add checkmark (`MFT_CHECKED`) for "Always on top" when enabled. Disable "Hide creature" when already hidden. Add "Next creature" only when multiple creatures exist.
- **Test:** `cargo test --workspace` passes. Menu reflects current state.
- **Rollback:** `git revert`.

### Task 4.6: Add render stats overlay
- **Files:** `crates/screenbuddy/src/main.rs`, `crates/screenbuddy/src/main_window.rs`
- **Changes:** Add a `show_stats` setting (toggle with F3). When enabled, draw a semi-transparent overlay with: FPS, frame time, draw call count, creature count, memory usage.
- **Test:** `cargo test --workspace` passes. F3 toggles stats overlay.
- **Rollback:** `git revert`.

### Task 4.7: Add shell restart recovery
- **Files:** `crates/screenbuddy-core/src/system_tray.rs`
- **Changes:** Register for `TaskbarCreated` message. When received, re-add the tray icon. This handles Explorer crashes/restarts.
- **Test:** `cargo test --workspace` passes. Tray icon recovers after Explorer restart.
- **Rollback:** `git revert`.

### Task 4.8: Add `WM_ERASEBKGND` handling
- **Files:** All window procs
- **Changes:** Handle `WM_ERASEBKGND` by returning `1` (prevents flash). This is a one-liner per window proc.
- **Test:** `cargo build --release` succeeds. No flicker on resize.
- **Rollback:** `git revert`.

### Task 4.9: Add `WM_DPICHANGED` handling
- **Files:** All window procs
- **Changes:** On `WM_DPICHANGED`, resize the window to the suggested rect. Re-layout all controls. Scale fonts and spacing by the new DPI.
- **Test:** `cargo build --release` succeeds. Window resizes correctly on DPI change.
- **Rollback:** `git revert`.

### Task 4.10: Unify color values
- **Files:** All UI files
- **Changes:** Replace all inline hex colors with theme constants. Ensure the same conceptual color uses the same value everywhere.
- **Test:** `cargo test --workspace` passes. Visual consistency.
- **Rollback:** `git revert`.

### Phase 4 Verification
- All Phase 1 + 2 + 3 tests still pass
- Theme system works (dark/light)
- IPC authentication works
- Tray: custom icon, balloons, dynamic menus, shell recovery
- Render stats overlay works
- No flicker on resize
- DPI change handled correctly

---

## Execution Rules

1. **One task at a time.** Complete a task, verify it passes all tests, commit, then move to the next.
2. **TDD where possible.** Write a failing test first, watch it fail, implement, watch it pass.
3. **No bundled refactors.** If a task reveals a related issue, note it but don't fix it in the same commit.
4. **Verify after each task.** Run the full probe suite after every commit.
5. **Commit message format:** `Fix <what> — <why>` or `Add <what> — <why>`.
6. **If a task fails 3 times:** Stop, diagnose root cause, question the approach before trying again.

## Test Suite (run after every task)

```bash
cargo build --release
cargo test --workspace
python tools/ipc_probe.py
python mcp/test_mcp.py
python tools/settings_probe.py
python tools/agent_probe.py
python tools/nine_router_probe.py
python tools/live_desktop_check.py
python tools/live_model_check.py
python tools/android_control_probe.py --serial GBNPCX062430B87
python tools/android_settings_probe.py --serial GBNPCX062430B87
python tools/test_ninerouter_management.py
```

## Estimated Timeline

| Phase | Tasks | Estimated Time |
|---|---|---|
| Phase 1 | 9 tasks | ~4 hours |
| Phase 2 | 4 tasks | ~2 hours |
| Phase 3 | 10 tasks | ~6 hours |
| Phase 4 | 10 tasks | ~4 hours |
| **Total** | **33 tasks** | **~16 hours** |
