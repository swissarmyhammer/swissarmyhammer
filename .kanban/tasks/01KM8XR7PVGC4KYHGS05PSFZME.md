---
assignees:
- claude-code
depends_on:
- 01KM8XQXTYWQ7R4EZ5PW8M2NZE
position_column: done
position_ordinal: ffffffffffffbc80
title: Route cancel_drag_session through dispatch_command
---
## What

`cancel_drag_session` is a Tauri command that clears the drag session from UIState and emits a drag-session-cancelled event. Route it through the command layer.

### Changes
- Add `drag.cancel` to YAML command definitions, `undoable: false`
- Add `DragCancelCmd` impl in `swissarmyhammer-kanban/src/commands/` — calls `ui.take_drag()`, returns cancelled session info
- The Tauri `dispatch_command` handler emits `drag-session-cancelled` as a post-execution side effect
- Frontend: replace `invoke(\"cancel_drag_session\", ...)` with `invoke(\"dispatch_command\", { cmd: \"drag.cancel\", args: { ... } })`
- Remove `cancel_drag_session` Tauri command from commands.rs and invoke_handler

### Tests (command layer — no GUI)
- [ ] `cargo nextest run -p swissarmyhammer-kanban` — test DragCancelCmd clears session from UIState
- [ ] Test cancelling when no session is active returns gracefully
- [ ] `cargo nextest run -p kanban-app` passes
- [ ] `pnpm --filter kanban-app test` passes