---
assignees:
- claude-code
depends_on:
- 01KMTJ0RPCMGC0TYASH3861JGZ
position_column: done
position_ordinal: ffffffffffffffc080
title: Wire native Edit menu Undo/Redo to command system
---
## What

The native Edit menu currently uses `PredefinedMenuItem::undo()` and `PredefinedMenuItem::redo()` which send the standard macOS undo/redo actions to the webview (for text fields). These don't route through our command system.

Replace the predefined items with custom `MenuItem`s that emit `menu-command` events with IDs `app.undo` and `app.redo`, which the frontend already routes through `executeCommand()`.

**Changes in `kanban-app/src/menu.rs`:**
- Replace `PredefinedMenuItem::undo(app, None)?` with `MenuItem::with_id(app, "app.undo", "Undo", true, Some("CmdOrCtrl+Z"))?`
- Replace `PredefinedMenuItem::redo(app, None)?` with `MenuItem::with_id(app, "app.redo", "Redo", true, Some("CmdOrCtrl+Shift+Z"))?`
- This makes them go through `handle_menu_event` → `menu-command` event → frontend `executeCommand("app.undo")` → `dispatch_command`

**Consideration:** When a CodeMirror editor is focused, Cmd+Z should do the editor's undo, not our global undo. The frontend command scope system already handles this — CM6 editors consume the keystroke before it bubbles to the command scope. The menu item will still fire, but the frontend `executeCommand` should check if an editor is focused and skip dispatch. This may need a small guard in the command dispatch logic.

**Files to modify:**
- `kanban-app/src/menu.rs` — replace predefined undo/redo with custom menu items

## Acceptance Criteria
- [ ] Edit > Undo fires `app.undo` command through dispatch system
- [ ] Edit > Redo fires `app.redo` command through dispatch system
- [ ] Menu items show Cmd+Z / Cmd+Shift+Z accelerators
- [ ] Cut/Copy/Paste/Select All still use predefined items (OS text handling)

## Tests
- [ ] Manual: Edit > Undo triggers undo of last entity mutation
- [ ] Manual: Edit > Redo triggers redo
- [ ] `cargo nextest run -p kanban-app` passes