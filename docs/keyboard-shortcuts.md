# ScreenBuddy Keyboard Shortcuts

## Main Window

| Key | Action |
|-----|--------|
| `Enter` | Send the message in the input box |
| `Shift+Enter` | Insert a newline in the input box (multiline) |
| `Backspace` | Edit the last sent message |
| `Esc` | Hide the window to the tray |
| `Ctrl+C` | Copy selected text from chat |
| `Ctrl+V` | Paste into the input box |
| `Ctrl+A` | Select all text in the input box |
| `Tab` | Move focus between UI regions |
| `Arrow Up/Down` | Navigate session list |
| `Arrow Left/Right` | Navigate settings tabs |

## Chat Window

| Key | Action |
|-----|--------|
| `Enter` | Send the message |
| `Shift+Enter` | Insert a newline |
| `Ctrl+C` | Copy selected text |
| `Ctrl+V` | Paste into the input box |
| `Ctrl+A` | Select all text |
| `Esc` | Close the chat window |
| `Page Up/Down` | Scroll through messages |
| `Home` | Jump to the first message |
| `End` | Jump to the last message |

## Settings Window

| Key | Action |
|-----|--------|
| `Tab` | Move focus to the next control |
| `Shift+Tab` | Move focus to the previous control |
| `Arrow Up/Down` | Adjust numeric values or navigate dropdowns |
| `Arrow Left/Right` | Adjust slider values |
| `Enter` | Activate the focused control |
| `Esc` | Close the settings window |
| `Ctrl+F` | Focus the search/filter box |

## Agent Editor

| Key | Action |
|-----|--------|
| `Tab` | Move focus to the next field |
| `Shift+Tab` | Move focus to the previous field |
| `Enter` | Activate the focused control |
| `Esc` | Close the editor without saving |
| `Ctrl+S` | Save the agent profile |

## Tray Menu

The tray icon is in the Windows notification area (system tray).

| Action | Result |
|--------|--------|
| Left-click icon | Open/restore the main window |
| Right-click icon | Show the context menu |

### Tray Context Menu

| Menu Item | Action |
|-----------|--------|
| Open ScreenBuddy | Show the main window |
| Minimise | Hide the window to the tray |
| Show Creature | Make the creature visible |
| Hide Creature | Hide the creature |
| Open Chat | Open the chat window |
| Next Creature | Switch to the next creature (only shown when 2+ creatures exist) |
| Open Agents | Open the agent editor |
| Open Settings | Open the settings window |
| Always on Top | Toggle window always-on-top (shows checkmark when active) |
| About | Show the About dialog |
| Quit ScreenBuddy | Exit the application |

## CLI Flags

| Flag | Action |
|------|--------|
| `--benchmark [duration] [creatures]` | Run headless performance benchmark |
| `--soak [duration] [interval]` | Run headless stability/soak test |
