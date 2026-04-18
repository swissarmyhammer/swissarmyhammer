---
assignees:
- claude-code
depends_on:
- 01KPG6G4SGXW1FN92YDXFNEAQ2
- 01KPG6GD34NMPQE1DZD0MHWE0N
- 01KPG6GN9JQSCZKFER5ZJ5JC62
- 01KPG6GYSNGTEJ42XA2QNB3VE0
- 01KPG6H74Z24N48DQR75CT7HP7
- 01KPG6HF1ZHWZ981PS3BEPP1HE
- 01KPG6HQYRRWCP52VH1KNKR35B
position_column: todo
position_ordinal: ec80
title: 'Commands: cut transactional safety — source deleted only after successful paste'
---
## What

Cross-cutting cut + paste is a two-step operation: copy the source onto the clipboard with `is_cut: true`, then on paste, create the destination AND delete the source. If paste fails partway, the source must remain intact. This card verifies that invariant across every handler that implements cut semantics.

### The invariant

For any cut-enabled handler (currently `TaskIntoColumnHandler`, `TaskIntoBoardHandler`, `TaskIntoProjectHandler`, `ColumnIntoBoardHandler` — not tag/actor/attachment since those are associations, not moves):

```
if is_cut:
    create_destination()  // must succeed first
    delete_source()       // only called after create succeeds
```

Fail modes to cover:

- `create_destination()` errors (disk full, invalid field, foreign key) → source remains, user can retry.
- `delete_source()` errors after a successful create → destination exists but source also still exists. Log a warning; don't roll back the create (the copy is real, the delete failure is recoverable).
- Handler panics mid-execute → source remains (panic unwinds before delete).

### Files to touch

- `swissarmyhammer-kanban/src/commands/paste_handlers/task_into_column.rs` / `task_into_board.rs` / `task_into_project.rs` / `column_into_board.rs` — enforce create-then-delete ordering; error-propagate from create; log-and-continue from delete.
- Shared test support (if card 01KPG6H fixtures lands) — or inline simulation of create failure via a mock `EntityContext`.

### Subtasks

- [ ] Audit each cut-enabled handler for create-then-delete ordering; refactor any that delete-first.
- [ ] Add per-handler test `{name}_cut_preserves_source_when_create_fails`.
- [ ] Add per-handler test `{name}_cut_succeeds_normally_deletes_source`.

## Acceptance Criteria

- [ ] Every cut-enabled handler creates first, then deletes.
- [ ] A simulated create failure (invalid field, nonexistent column, etc.) leaves the source entity intact.
- [ ] Successful paste-cut deletes the source exactly once.
- [ ] Delete failure after successful create does NOT roll back the create.

## Tests

- [ ] `task_into_column_cut_preserves_source_when_create_fails` — force create error (nonexistent column id), assert source task still present.
- [ ] `task_into_board_cut_preserves_source_when_create_fails` — board with no columns raises create error; source stays.
- [ ] `task_into_project_cut_preserves_source_when_create_fails`.
- [ ] `column_into_board_cut_preserves_source_when_create_fails`.
- [ ] One test per handler confirming the happy path deletes the source exactly once.
- [ ] Run command: `cargo nextest run -p swissarmyhammer-kanban paste_handlers` — all green.

## Workflow

- Use `/tdd` — write the failure-path test for one handler first; confirm it fails on naive implementations; fix; repeat.

#commands

Depends on: all 7 paste handlers