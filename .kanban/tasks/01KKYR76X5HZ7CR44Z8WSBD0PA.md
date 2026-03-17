---
assignees:
- assistant
depends_on:
- 01KKYR690S88SENAN2HVB6X5BJ
position_column: todo
position_ordinal: '8380'
title: Ensure window_boards board paths are auto-opened on startup
---
## What
In `auto_open_board`, after restoring boards from `config.open_boards`, scan `config.window_boards` for any board paths not yet in the `boards` map and open them. This is a safety net: if `open_boards` and `window_boards` drift out of sync, the boards needed by secondary windows will still be available when `restore_windows` runs.

### Files
- `kanban-app/src/state.rs` — `auto_open_board` method (lines 431-536), add window_boards scan after open_boards restoration

### Subtasks
- [ ] After the `open_boards` restoration loop, collect board paths from `window_boards`
- [ ] Filter to paths not already in `boards` map and that exist on disk
- [ ] Open each missing board
- [ ] `cargo nextest run` passes

## Acceptance Criteria
- [ ] If `window_boards` references a board not in `open_boards`, it gets opened on startup
- [ ] Non-existent board paths in `window_boards` are skipped gracefully (log warning)
- [ ] No duplicate opens if board is already in both `open_boards` and `window_boards`

## Tests
- [ ] `cargo nextest run` — full suite green
- [ ] Manual: edit config.json to have a window_boards entry for a board not in open_boards, launch app — board should open and window should restore