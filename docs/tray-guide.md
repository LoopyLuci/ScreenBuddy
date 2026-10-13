# ScreenBuddy Tray Guide

## Tray Icon

The ScreenBuddy tray icon lives in the Windows notification area (system tray),
typically at the bottom-right corner of the screen.

### Icon States

| State | Meaning |
|-------|---------|
| Normal | App is running, creature is visible |
| Dimmed | App is running, creature is hidden |
| Flashing | New message or notification |

## Left-Click

Left-clicking the tray icon opens or restores the main window. If the window
is already visible and focused, left-clicking minimizes it to the tray.

## Right-Click Context Menu

Right-clicking the tray icon shows the context menu with the following items:

### Open ScreenBuddy

Shows the main window. If the window is hidden or minimized, it is restored
and brought to the foreground.

### Minimise

Hides the main window to the tray. The app continues running in the background.
This is not the same as quitting — the creature keeps animating and the app
keeps responding to IPC commands.

### Show Creature / Hide Creature

Toggles creature visibility. When hidden, the creature is not rendered but
the app continues running. The menu item reflects the current state:
- **Show Creature** appears when the creature is currently hidden
- **Hide Creature** appears when the creature is currently visible

### Open Chat

Opens the chat window. This is a separate window from the main window and
can be used independently.

### Next Creature

Switches to the next creature in the rotation. This menu item only appears
when two or more creatures are loaded. With a single creature, the item is
hidden because there is nothing to switch to.

### Open Agents

Opens the agent editor. Agents are profiles that bundle a model, persona,
character, and behaviour settings. The editor has three tabs:
- **Identity** — name, character, persona, instructions
- **Model** — provider, model, temperature, tool steps, timeout
- **Behaviour** — movement, tools, presets

### Open Settings

Opens the settings window. Six tabs cover all app settings. Every setting is
either live (takes effect immediately), read-only (reflects runtime state),
or visibly disabled (unwired, with an explanation).

### Always on Top

Toggles whether the main window stays above all other windows. When active,
a checkmark appears next to the menu item. The state is reflected in the
settings window and persists across restarts.

### About

Shows the About dialog with version information, credits, and license details.

### Quit ScreenBuddy

Exits the application completely. This:
1. Removes the tray icon
2. Saves all settings and sessions
3. Closes all windows
4. Terminates the process

This is the only way to actually exit the app. Closing the window or
minimizing it does not quit — it hides to the tray.

## Window Behaviour

### Minimize vs Close

Both the minimize button and the close button hide the window to the tray.
This is intentional: a desktop companion that vanished when its window
closed looked like a crash. To actually quit, use **Quit ScreenBuddy** in
the tray menu.

### Session List

The sidebar in the main window lists every session with:
- Message count
- Auto-generated title from the first message
- Timestamp of last activity

Click a session to switch to it. Use **+ New chat** to create a new session,
and the per-row **-** button to delete one.

### Message Input

The message input box at the bottom of the main window supports:
- **Enter** to send
- **Shift+Enter** for multiline input
- **Backspace** to edit the last sent message
- **Ctrl+C** to copy selected text
- **Ctrl+V** to paste

## Keyboard Navigation

All windows support keyboard navigation:
- **Tab** moves focus between controls
- **Arrow keys** navigate lists and adjust values
- **Enter** activates the focused control
- **Esc** closes the current window or dialog

See [Keyboard Shortcuts](keyboard-shortcuts.md) for the full reference.
