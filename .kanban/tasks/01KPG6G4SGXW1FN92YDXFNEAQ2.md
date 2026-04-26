---
assignees:
- claude-code
depends_on:
- 01KPG5YB7GTQ6Q3CEQAMXPJ58F
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffe880
title: 'Commands: paste handler — task onto column'
---
## What

Implement `TaskIntoColumnHandler` — pastes a task from the clipboard into a target column. Handler lives at `swissarmyhammer-kanban/src/commands/paste_handlers/task_into_column.rs`, matches `("task", "column")`, registered via `register_paste_handlers()` in the mechanism card (01KPG5YB7GTQ6Q3CEQAMXPJ58F).

### Action

1. Parse column id from the target moniker (`column:todo` → `"todo"`).
2. Clone `clipboard.fields`; set `column` field to the target column id; drop any stale `ordinal` so `AddEntity`'s position helper appends to the end.
3. Invoke `AddEntity::new("task").with_overrides(fields)` via `run_op`.
4. If `clipboard.is_cut`, dispatch `DeleteTask::new(&clipboard.entity_id)` afterward.

### Files

- CREATE `swissarmyhammer-kanban/src/commands/paste_handlers/task_into_column.rs` — struct, `PasteHandler` impl, colocated tests.
- MODIFY `swissarmyhammer-kanban/src/commands/paste_handlers/mod.rs` — add `m.register(TaskIntoColumnHandler);` in `register_paste_handlers()`.

### Subtasks

- [x] Implement `TaskIntoColumnHandler` struct and `PasteHandler` impl.
- [ ] Register in `register_paste_handlers()` — **DEFERRED**: registration line `m.register(TaskIntoColumnHandler);` deferred per parallel-safety override; orchestrator will batch-register after all sibling handler files exist.
- [x] Colocate unit tests in the same file.

## Acceptance Criteria

- [x] Handler matches `("task", "column")`.
- [x] Pasting a task onto a column creates a new task in that column with the clipboard's field values.
- [x] Cut variant (is_cut=true) deletes the source task after successful create.
- [ ] `PasteMatrix::find("task", "column")` returns this handler — **DEFERRED**: pending orchestrator batch-registration. Verified via local test matrix in `local_matrix_finds_task_into_column_handler`.

## Tests

- [x] `paste_task_into_column_creates_copy` — colocated. Create source task, copy it, paste into a different column, assert new task exists with source's fields and correct column.
- [x] `paste_task_into_column_preserves_fields` — synthetic snapshot with tags/assignees/project, paste, assert those fields carried over and position fields are overridden by the target column.
- [x] `paste_cut_task_deletes_source` — cut path deletes source.
- [x] Run command: `cargo nextest run -p swissarmyhammer-kanban paste_handlers::task_into_column` — all 9 tests green.

Plus extra colocated tests:
- `local_matrix_finds_task_into_column_handler` — verifies `PasteMatrix::find` resolves us when registered locally.
- `handler_matches_returns_task_column_pair` — pins the matches() pair.
- `paste_into_non_column_target_errors` — non-column moniker rejected loudly.
- `snapshot_position_keys_are_overridden_by_target_column` — guards `POSITION_KEYS_TO_DROP`.
- `handler_available_defaults_to_true` — regression guard so a future override does not silently disable all (task, column) pastes.
- `task_id_round_trips_through_delete_op_constructor` — compile-time safety net for the cut path's `DeleteTask::new(impl Into<TaskId>)` signature.

## Workflow

- Use `/tdd` — write `paste_task_into_column_creates_copy` first.

#commands

Depends on: 01KPG5YB7GTQ6Q3CEQAMXPJ58F (mechanism)