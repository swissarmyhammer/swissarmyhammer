---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
title: Add done_tasks and percent_complete to BoardSummary in Rust backend
---
## What
Add `done_tasks` (count of tasks in the terminal column) and `percent_complete` (done/total * 100, integer) to the summary JSON returned by both:
1. **Tauri `get_board_data`** in `kanban-app/src/commands.rs` (lines 566-587) — already has `terminal_id` and `column_counts`, just needs to read done count from those
2. **Kanban `GetBoard::execute`** in `swissarmyhammer-kanban/src/board/get.rs` (lines 170-195) — same pattern, already has `column_counts` and `terminal_id`

The terminal column is determined by `columns.last()` (highest order) — this is already computed in both places. `done_tasks = column_counts[terminal_id]`.

## Acceptance Criteria
- [ ] `summary.done_tasks` is present in the JSON returned by `get_board_data`
- [ ] `summary.percent_complete` is present (0 when no tasks, integer 0-100)
- [ ] Both Tauri command and kanban crate `GetBoard` return these fields consistently
- [ ] Existing tests still pass

## Tests
- [ ] Update `test_empty_board` in `get.rs` to assert `done_tasks: 0, percent_complete: 0`
- [ ] Update `test_board_with_tasks_in_different_columns` to assert `done_tasks: 1, percent_complete: 33`
- [ ] Run `cargo nextest run -p swissarmyhammer-kanban` — all pass