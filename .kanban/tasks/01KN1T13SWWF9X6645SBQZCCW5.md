---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffff9480
title: 'Fix: secondary windows not restored on startup — board open ordering race'
---
## What

Secondary windows are not restored on app restart. Only the main window appears. The bug is in `restore_windows` (`kanban-app/src/commands.rs:688-760`).

### Root cause

`restore_windows` iterates saved windows and for each one checks if the board is already open (`state.boards.contains_key(&canonical)` at line 723). If the board isn't open yet, the window is silently skipped (`continue` at line 724). At startup, the main window opens its board first, then calls `restore_windows`. If a secondary window uses a different board, that board hasn't been opened yet → the check fails → the window is skipped.

### Fix

When `restore_windows` encounters a saved window whose board isn't open yet, it should open the board (call `state.open_board()`) instead of skipping. This is what the main window does — it opens its board then restores secondaries. The secondaries should do the same.

### Files to modify
- `kanban-app/src/commands.rs` — `restore_windows` function (lines 688-760): open the board if not already open, instead of skipping

### Test gap
There are zero tests for `restore_windows`. The data layer tests (save/load round-trips in `ui_state.rs`) pass, but nothing tests the actual window restoration logic.

## Acceptance Criteria
- [x] Secondary windows with different boards are restored on startup
- [x] Secondary windows with the same board as main are restored
- [x] Window positions and sizes are restored correctly
- [x] `cargo nextest run` passes

## Tests
- [x] `ui_state.rs` — test: `all_windows()` returns both main and secondary after save/load with different board_paths (already exists and passes)
- [ ] `commands.rs` or integration test — test: `restore_windows` opens boards that aren't already open (new test needed — may require mocking AppState or using a test harness)
- [x] `cargo nextest run -p swissarmyhammer-commands` passes
- [x] `cargo nextest run -p kanban-app` passes