---
assignees:
- claude-code
position_column: todo
position_ordinal: '8180'
title: Route set_focus and all drag commands through dispatch_command
---
## What

Four Tauri commands should be regular commands dispatched through `dispatch_command`: `set_focus`, `start_drag_session`, `cancel_drag_session`, and `complete_drag_session`. All state mutations go through the command layer — no exceptions.

### Changes
- Add YAML command definitions: `ui.setFocus`, `drag.start`, `drag.cancel`, `drag.complete`
- Add command impls that call existing UIState methods / dispatch `task.move` internally
- `drag.complete` replaces the Tauri command — it reads the drag session from UIState, then dispatches `task.move` or handles cross-board transfer
- **Cross-board transfer logic moves into the command engine** (`task.transfer` or `drag.complete` impl in swissarmyhammer-kanban). This makes it testable without Tauri/GUI — write real integration tests for same-board move, cross-board copy, cross-board move.
- Frontend: replace all four `invoke(...)` calls with `invoke("dispatch_command", { cmd: "...", ... })`
- Remove all four Tauri commands from commands.rs and invoke_handler
- Remove the `cross_board_transfer` helper from commands.rs (moved to command engine)
- Mark all as `undoable: false` in YAML (transient state, except `drag.complete` which dispatches an undoable `task.move`)

### Testing
Cross-board transfer is complex (read source, copy fields, compute ordinal, create on target, delete source). As a command impl it's testable without GUI:
- Same-board move via drag.complete
- Cross-board copy (task appears on target, stays on source)
- Cross-board move (task appears on target, removed from source)
- Field copying (title, body, assignees carry over; tags filtered to target board)

## Acceptance Criteria
- [ ] All four Tauri commands removed
- [ ] All four routed through dispatch_command
- [ ] Cross-board transfer logic in command engine, not Tauri layer
- [ ] Focus, drag start, drag cancel, drag complete still work
- [ ] Cross-window drag still works
- [ ] No functional regression

## Tests
- [ ] `cargo nextest run -p kanban-app` passes
- [ ] `cargo nextest run -p swissarmyhammer-kanban` — cross-board transfer tests
- [ ] `pnpm --filter kanban-app test` passes