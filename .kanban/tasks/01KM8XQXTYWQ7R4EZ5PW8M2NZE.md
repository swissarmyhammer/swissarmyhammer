---
assignees:
- claude-code
depends_on:
- 01KM8XQH7MV5YCP8QF93K1MQR0
position_column: done
position_ordinal: ffffffffffeb80
title: Route start_drag_session through dispatch_command
---
## What

`start_drag_session` is a Tauri command that stores a DragSession in UIState and emits a drag-session-active event. Route it through the command layer.

### Changes
- Add `drag.start` to YAML command definitions (`file.yaml` or new `drag.yaml`), `undoable: false`
- Add `DragStartCmd` impl in `swissarmyhammer-kanban/src/commands/` — calls `ui.start_drag()`, returns the session info
- The Tauri `dispatch_command` handler emits `drag-session-active` as a post-execution side effect when it sees a DragStart result (same pattern as BoardSwitch)
- Frontend: replace `invoke(\"start_drag_session\", ...)` with `invoke(\"dispatch_command\", { cmd: \"drag.start\", args: { ... } })`
- Remove `start_drag_session` Tauri command from commands.rs and invoke_handler

### Tests (command layer — no GUI)
- [ ] `cargo nextest run -p swissarmyhammer-kanban` — test DragStartCmd stores session in UIState
- [ ] Test that starting a new drag cancels any existing session
- [ ] `cargo nextest run -p kanban-app` passes
- [ ] `pnpm --filter kanban-app test` passes