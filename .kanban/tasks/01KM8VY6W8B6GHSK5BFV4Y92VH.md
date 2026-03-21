---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffe780
title: Route complete_drag_session entity moves through dispatch_command
---
## What

`complete_drag_session` is a Tauri command that performs entity moves (task.move, column.reorder) inline without going through `dispatch_command`. This means drag-and-drop moves are unundoable.

### Current behavior
The command reads the drag session from UIState, then directly calls entity operations (move task to column, reorder columns) via the board handle's processor. The move result never passes through the command system.

### Fix
After extracting the move parameters from the drag session, dispatch `task.move` or `column.reorder` through `dispatch_command` (or directly invoke the command impl). The drag session management (start/cancel/take) stays in UIState, but the actual entity mutation goes through the command system.

## Acceptance Criteria
- [ ] Drag-and-drop task moves go through dispatch_command
- [ ] Drag-and-drop column reorders go through dispatch_command
- [ ] Drag session start/cancel remain as direct Tauri commands (transient UI state)
- [ ] Cross-window drag still works

## Tests
- [ ] `cargo nextest run -p kanban-app` passes
- [ ] Manual: drag a task, verify it moves correctly