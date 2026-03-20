---
assignees:
- claude-code
depends_on:
- 01KM621VQF672VCZ26S8DG350S
- 01KM624X14YB50494X8506YA5E
position_column: done
position_ordinal: ffffffffffb380
title: 'Test: board counts and MCP operations exclude archived entities'
---
## What

Integration tests verifying that archived entities are invisible across all user-facing paths. The directory-based separation means `list()` inherently excludes them, but these tests lock in that guarantee at the integration level — if someone ever changes the storage model, these tests catch the regression.

### Test scenarios

**Board counts (GetBoard):**
- Create 3 tasks, archive 1 → `total_tasks` should be 2
- Archive a done task → `done_tasks` count decreases
- Archive a ready task → `ready_tasks` count decreases

**NextTask:**
- Create 2 tasks, archive the first → `next task` returns the second
- Archive all tasks → `next task` returns null

**ListTasks:**
- Create 3 tasks, archive 1 → `list tasks` returns 2
- Archive + unarchive → task reappears in list

**Tauri list_entities (if testable):**
- This is the Tauri command that calls `ectx.list()` directly
- If not directly testable, verify through `KanbanContext::list_entities_generic()` which wraps it

**MCP kanban tool:**
- `list tasks` via the MCP operation processor → archived excluded
- `next task` via MCP → archived excluded

### Files
- `swissarmyhammer-kanban/src/board/get.rs` — GetBoard tests
- `swissarmyhammer-kanban/src/task/next.rs` — NextTask tests
- `swissarmyhammer-kanban/src/task/list.rs` — ListTasks tests

## Acceptance Criteria
- [ ] GetBoard counts exclude archived tasks
- [ ] NextTask skips archived tasks
- [ ] ListTasks excludes archived tasks
- [ ] Unarchived tasks reappear in all of the above

## Tests
- [ ] `test_get_board_excludes_archived_from_counts` — archive task, verify counts
- [ ] `test_next_task_skips_archived` — archive first task, next returns second
- [ ] `test_next_task_all_archived_returns_null` — archive all, returns null
- [ ] `test_list_tasks_excludes_archived` — archive 1 of 3, list returns 2
- [ ] `test_list_tasks_unarchive_restores` — archive then unarchive, task reappears
- [ ] `cargo test -p swissarmyhammer-kanban`