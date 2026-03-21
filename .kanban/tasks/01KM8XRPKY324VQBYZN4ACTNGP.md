---
assignees:
- claude-code
depends_on:
- 01KM8XR7PVGC4KYHGS05PSFZME
position_column: todo
position_ordinal: '8480'
title: Route complete_drag_session through dispatch_command with cross-board transfer in command engine
---
## What

`complete_drag_session` is a Tauri command that takes the drag session, then dispatches task.move (same-board) or does direct entity manipulation (cross-board). Move ALL of this into the command engine.

### Changes
- Add `drag.complete` to YAML command definitions, `undoable: false` (the inner task.move IS undoable)
- Add `DragCompleteCmd` impl in `swissarmyhammer-kanban/src/commands/` that:
  - Reads drag session from UIState
  - Same-board: calls the task.move command impl directly via CommandContext
  - Cross-board: implements transfer logic (read source, copy fields, create on target, optionally delete source)
- Move `cross_board_transfer` logic from `kanban-app/src/commands.rs` into the command impl
- The Tauri `dispatch_command` handler emits `drag-session-completed` as a post-execution side effect
- Frontend: replace `invoke(\"complete_drag_session\", ...)` with `invoke(\"dispatch_command\", { cmd: \"drag.complete\", args: { ... } })`
- Remove `complete_drag_session` Tauri command and `cross_board_transfer` helper from commands.rs
- Remove from invoke_handler

### Tests (command layer — no GUI)
All tests use KanbanContext directly, no Tauri:
- [ ] Same-board move via drag.complete — task moves to target column
- [ ] Same-board move with before_id/after_id — correct ordinal placement
- [ ] Cross-board move — task appears on target, removed from source
- [ ] Cross-board copy — task appears on target, stays on source
- [ ] Field copying — title, body, assignees carry over
- [ ] Tag filtering — source tags not on target board are stripped
- [ ] No active drag session — returns error gracefully
- [ ] `cargo nextest run -p swissarmyhammer-kanban` passes
- [ ] `cargo nextest run -p kanban-app` passes
- [ ] `pnpm --filter kanban-app test` passes