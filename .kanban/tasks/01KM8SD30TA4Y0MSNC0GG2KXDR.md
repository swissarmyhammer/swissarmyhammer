---
assignees:
- claude-code
depends_on:
- 01KM8PZM4PDZS7A4FKWDQ34ZHZ
position_column: done
position_ordinal: ffffffffffe080
title: Move active board into per-window WindowState
---
## What

Board assignment is tracked in two places: `UIState.window_boards` (per-window map of label → path) and `UIState.active_board_path` (global singleton). The window_boards map is redundant with WindowState — board assignment should just be another field on WindowState, like active_view_id and inspector_stack.

### Changes
- Add `board_path: String` to `WindowState` in UIState
- Remove `window_boards: HashMap<String, String>` from UIStateInner (redundant with windows map)
- Remove `active_board_path` from UIStateInner (meaningless with multi-window — the `most_recent_board_path` card handles the quick-capture use case separately)
- Update `set_window_board()` to write to `windows[label].board_path`
- Update `window_board()` to read from `windows[label].board_path`
- Remove `add_open_board()` / `set_active_board_path()` — board path is set via `set_window_board()`
- Keep `open_boards` list for tracking which boards are loaded (distinct from which window shows which board)
- Update all callers: dispatch_command, restore_windows, auto_open_board, file_commands, etc.

## Acceptance Criteria
- [ ] `window_boards` map removed (board_path is in WindowState)
- [ ] `active_board_path` removed
- [ ] Each window's board assignment persists in WindowState
- [ ] Board open/close/switch still work
- [ ] Window restore still works

## Tests
- [ ] `cargo nextest run -p kanban-app -p swissarmyhammer-commands` passes
- [ ] `pnpm --filter kanban-app test` passes