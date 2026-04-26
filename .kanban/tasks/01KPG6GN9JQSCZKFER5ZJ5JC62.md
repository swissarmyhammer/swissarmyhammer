---
assignees:
- claude-code
depends_on:
- 01KPG5YB7GTQ6Q3CEQAMXPJ58F
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffed80
title: 'Commands: paste handler — task onto project'
---
## What

Implement `TaskIntoProjectHandler` — pastes a task with its `project` field set to the target project. Handler at `swissarmyhammer-kanban/src/commands/paste_handlers/task_into_project.rs`, matches `("task", "project")`.

### Action

1. Parse project id from the target moniker.
2. Clone `clipboard.fields`; set `project` field to the target project id.
3. If the clipboard source had a `column`, preserve it; otherwise fall back to the first column of the board the project is being pasted in (use `resolve_column` from `entity/position.rs` if it supports this case, or query directly).
4. Invoke `AddEntity::new("task").with_overrides(fields)`.
5. If `clipboard.is_cut`, delete the source task.

Note: whether a pasted task picks up the destination project's implicit column depends on how projects are rendered in the current view. For v1, set `project` and let the existing column stand; product can refine later.

### Files

- CREATE `swissarmyhammer-kanban/src/commands/paste_handlers/task_into_project.rs`.
- MODIFY `swissarmyhammer-kanban/src/commands/paste_handlers/mod.rs` — register call.

### Subtasks

- [x] Implement `TaskIntoProjectHandler`.
- [x] Register. (Note: `m.register(TaskIntoProjectHandler);` deferred — orchestrator will batch-register. `pub mod task_into_project;` added to `mod.rs` per parallel-safety override.)
- [x] Colocate tests.

## Acceptance Criteria

- [x] Handler matches `("task", "project")`.
- [x] Pasting a task onto a project creates a new task with `project: <id>` set.
- [x] Source task's `column` is preserved if set; if not set, falls back gracefully (to leftmost column via AddEntity).
- [x] Cut variant deletes the source task.

## Tests

- [x] `paste_task_into_project_sets_project_field` — assert the new task's `project` equals the target id.
- [x] `paste_task_into_project_preserves_source_column` — source had `column: doing`; new task also has `column: doing`.
- [x] `paste_cut_task_into_project_deletes_source`.
- [x] Run command: `cargo nextest run -p swissarmyhammer-kanban paste_handlers::task_into_project` — all green (10/10 passed).

Additional tests added for robustness:
- `local_matrix_finds_task_into_project_handler`
- `handler_matches_returns_task_project_pair`
- `paste_into_non_project_target_errors`
- `paste_task_into_project_falls_back_to_leftmost_column`
- `paste_task_into_project_recomputes_ordinal`
- `paste_task_into_project_overrides_snapshot_project`
- `handler_available_defaults_to_true`

## Workflow

- Use `/tdd`.

#commands

Depends on: 01KPG5YB7GTQ6Q3CEQAMXPJ58F (mechanism)