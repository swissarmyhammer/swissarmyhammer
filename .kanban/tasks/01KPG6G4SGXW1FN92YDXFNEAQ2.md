---
assignees:
- claude-code
depends_on:
- 01KPG5YB7GTQ6Q3CEQAMXPJ58F
position_column: todo
position_ordinal: de80
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

- [ ] Implement `TaskIntoColumnHandler` struct and `PasteHandler` impl.
- [ ] Register in `register_paste_handlers()`.
- [ ] Colocate unit tests in the same file.

## Acceptance Criteria

- [ ] Handler matches `("task", "column")`.
- [ ] Pasting a task onto a column creates a new task in that column with the clipboard's field values.
- [ ] Cut variant (is_cut=true) deletes the source task after successful create.
- [ ] `PasteMatrix::find("task", "column")` returns this handler.

## Tests

- [ ] `paste_task_into_column_creates_copy` — colocated. Create source task, copy it, paste into a different column, assert new task exists with source's fields and correct column.
- [ ] `paste_task_into_column_preserves_fields` — copy a task with tags/assignees/project, paste, assert those fields carried over.
- [ ] `paste_cut_task_deletes_source` — cut path deletes source.
- [ ] Run command: `cargo nextest run -p swissarmyhammer-kanban paste_handlers::task_into_column` — all green.

## Workflow

- Use `/tdd` — write `paste_task_into_column_creates_copy` first.

#commands

Depends on: 01KPG5YB7GTQ6Q3CEQAMXPJ58F (mechanism)