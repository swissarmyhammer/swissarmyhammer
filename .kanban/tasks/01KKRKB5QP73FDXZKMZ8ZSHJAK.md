---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffaa80
title: 'Tauri: add global-shortcut plugin and quick-capture window backend'
---
## What

Add the Tauri `global-shortcut` plugin and register a configurable global hotkey (default `Cmd+Shift+K`) that shows/hides a small quick-capture window. The window is created once at startup (hidden) and toggled on hotkey.

**Affected files:**
- `kanban-app/Cargo.toml` — add `tauri-plugin-global-shortcut = "2"` dependency
- `kanban-app/tauri.conf.json` — add second window definition for `quick-capture` (small, borderless, always-on-top, hidden, centered)
- `kanban-app/src/main.rs` — register `tauri_plugin_global_shortcut`, register the hotkey in `setup()`, toggle `quick-capture` window visibility
- `kanban-app/capabilities/default.json` (or equivalent) — grant `global-shortcut:default` permission

**Approach:**
- Add a second window entry in `tauri.conf.json`:
  ```json
  {
    "label": "quick-capture",
    "title": "",
    "width": 400,
    "height": 120,
    "resizable": false,
    "decorations": false,
    "alwaysOnTop": true,
    "visible": false,
    "center": true,
    "skipTaskbar": true
  }
  ```
- In `setup()`, register `Cmd+Shift+K` via `app.global_shortcut().on_shortcut(...)` to toggle the window:
  - If hidden → show, center, focus
  - If visible → hide
- The quick-capture window loads the same frontend dist but with a URL query param (e.g. `?window=quick-capture`) so the React app can render the capture UI instead of the main board
- The `task.add` command already exists and accepts `column` as an arg — the frontend will call `dispatch_command` with `cmd: "task.add"` and `args: { column: "<first-column-id>", title: "<text>" }`

## Acceptance Criteria
- [ ] `tauri-plugin-global-shortcut` is added to Cargo.toml and registered in `main.rs`
- [ ] A `quick-capture` window is defined in `tauri.conf.json` (borderless, always-on-top, ~400x120)
- [ ] `Cmd+Shift+K` toggles the quick-capture window visibility
- [ ] Window appears centered on the active screen
- [ ] Window hides when hotkey is pressed again
- [ ] The quick-capture window loads the frontend with a distinguishing parameter

## Tests
- [ ] `cargo build` succeeds with the new plugin
- [ ] Manual test: press `Cmd+Shift+K` → window appears centered, press again → hides
- [ ] Manual test: the main window continues to work normally
- [ ] `cargo nextest run` passes for kanban-app