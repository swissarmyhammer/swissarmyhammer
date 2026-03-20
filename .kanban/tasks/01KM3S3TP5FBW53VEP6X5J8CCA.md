---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffff8280
title: close_board should not remove window entries — window state is independent of board
---
## What
`close_board` calls `config.windows.retain(|_, entry| entry.board_path != canonical)` which deletes window entries when their board is closed. This is wrong — window geometry belongs to the window, not the board. A window can have no board loaded.

**Current behavior:** Close a board → window entry deleted → geometry lost → frontend creates fresh entry with null geometry via switch_board.

**Correct behavior:** Close a board → window entry stays, board_path cleared (or left stale) → frontend calls switch_board to update board_path to the fallback → geometry preserved.

**Fix:**
1. `close_board` in `commands.rs`: remove the `config.windows.retain(...)` call entirely. The windows map tracks windows, not boards. When a board closes, the window still exists.
2. The `WindowState.board_path` for affected windows becomes stale, but the frontend immediately calls `switch_board` with the fallback board (or clears state if no boards remain), which updates `board_path`.
3. For secondary windows that are actually destroyed (mid-session close), the `on_window_event` Destroyed handler already removes `config.windows[label]` — that's correct because the window itself is gone.

**Separation of concerns:**
- `config.windows` = window lifecycle (create/destroy/geometry)
- `config.open_boards` = which boards are loaded in the backend
- `windows[label].board_path` = which board a window is currently showing (updated by switch_board)

## Tests
- [ ] Unit test: close_board does NOT remove any window entries
- [ ] Unit test: close_board only updates open_boards list
- [ ] Unit test: window geometry survives board close + switch_board to fallback
- [ ] Manual: close a board, verify window doesn't jump/resize
- [ ] `cargo nextest run -p kanban-app` passes

## Acceptance Criteria
- [ ] close_board never removes entries from config.windows
- [ ] Window geometry preserved when board is closed and window falls back
- [ ] Secondary window Destroyed event still cleans up its entry (unchanged)