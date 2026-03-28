---
assignees:
- claude-code
position_column: todo
position_ordinal: c380
title: Board progress ring does not update after drag-to-done
---
## What

After dragging a task to the done column, the board progress ring in the NavBar stays stale. The backend computes `percent_complete` correctly in `GetBoard` (`swissarmyhammer-kanban/src/board/get.rs:174-183`), but the UI never re-fetches `BoardData` after a task move.

**Root cause**: In `App.tsx`, the `entity-field-changed` event handler for tasks (lines ~384-385) only updates the local entity store — it does not call `refresh()` which would invoke `get_board_data` and recompute the summary. The `refresh()` call only happens for structural changes (column/swimlane add/remove), not task field changes.

**Fix**: In `board-view.tsx`, `persistMove()` (lines ~207-211) dispatches `task.move` but does not call `refresh()` after the move succeeds. Add a `refresh()` call after `persistMove` completes. This triggers `get_board_data` which recomputes `percent_complete` from the database.

Alternatively, the `entity-field-changed` handler in `App.tsx` could detect `position_column` changes and trigger a refresh — but that's broader than needed. The simplest fix is to refresh after `persistMove`.

### Files to modify
- `kanban-app/ui/src/components/board-view.tsx` — call `refresh()` after `persistMove()` succeeds

## Acceptance Criteria
- [ ] Dragging a task to the done column immediately updates the progress ring
- [ ] Dragging a task out of done immediately updates the progress ring
- [ ] No extra network calls when non-position fields change

## Tests
- [ ] Manual: drag task to done → progress ring updates
- [ ] Manual: drag task out of done → progress ring decreases
- [ ] Run: `cd kanban-app/ui && npm test` — no regressions"