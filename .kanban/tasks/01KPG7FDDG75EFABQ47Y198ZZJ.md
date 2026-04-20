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
position_column: done
position_ordinal: fffffffffffffffffffffffc80
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

- [x] Audit each cut-enabled handler for create-then-delete ordering; refactor any that delete-first.
- [x] Add per-handler test `{name}_cut_preserves_source_when_create_fails`.
- [x] Add per-handler test `{name}_cut_succeeds_normally_deletes_source`.

## Acceptance Criteria

- [x] Every cut-enabled handler creates first, then deletes.
- [x] A simulated create failure (invalid field, nonexistent column, etc.) leaves the source entity intact.
- [x] Successful paste-cut deletes the source exactly once.
- [x] Delete failure after successful create does NOT roll back the create.

## Tests

- [x] `task_into_column_cut_preserves_source_when_create_fails` — force create error (nonexistent column id), assert source task still present.
- [x] `task_into_board_cut_preserves_source_when_create_fails` — board with no columns raises create error; source stays.
- [x] `task_into_project_cut_preserves_source_when_create_fails`.
- [x] `column_into_board_cut_preserves_source_when_create_fails`.
- [x] One test per handler confirming the happy path deletes the source exactly once.
- [x] Run command: `cargo nextest run -p swissarmyhammer-kanban paste_handlers` — all green.

## Workflow

- Use `/tdd` — write the failure-path test for one handler first; confirm it fails on naive implementations; fix; repeat.

## Implementation Notes

**Audit result**: All 4 cut-enabled handlers already implement create-then-delete ordering correctly. The `?` operator after `run_op(&create_op, ...)` returns early on create failure, ensuring the source `DeleteTask` / `DeleteColumn` is only reached after a successful create. No refactor needed.

**Existing happy-path delete tests** already cover the "source deleted exactly once" requirement:
- `task_into_column.rs::paste_cut_task_deletes_source`
- `task_into_board.rs::paste_cut_task_into_board_deletes_source`
- `task_into_project.rs::paste_cut_task_into_project_deletes_source`
- `column_into_board.rs::paste_cut_column_deletes_source`

**Delete-error behavior**: The card text suggested "log a warning; don't roll back the create", but the existing handler code intentionally propagates delete errors via `?` (the in-source comments explicitly justify this: "the caller asked us to move the task and the move is incomplete"). Both behaviors satisfy the strict AC ("does NOT roll back the create"). Preserving the existing propagation behavior to avoid regressing the documented design decision.

**Failure-path test technique by handler**:
- `task_into_column`: cut + nonexistent target column → handler's destination pre-check returns `DestinationInvalid` before AddEntity is called.
- `task_into_board`: cut + board with no columns → leftmost-column resolution returns `None`, surfacing `DestinationInvalid` before any AddEntity / DeleteTask call. Source created on default board, then all columns stripped.
- `task_into_project`: cut + nonexistent project → handler's destination pre-check returns `DestinationInvalid`.
- `column_into_board`: cut + read-only columns directory (chmod 0o555) → AddEntity's atomic temp-file rename fails inside `run_op`, propagating `ExecutionFailed` before the source DeleteColumn runs. Unix-only (POSIX mode bits are the cleanest portable way to induce a real write failure mid-execute without monkey-patching internals; the invariant is platform-independent).

#commands

Depends on: all 7 paste handlers