---
assignees:
- claude-code
depends_on:
- 01KPG5YB7GTQ6Q3CEQAMXPJ58F
position_column: todo
position_ordinal: e080
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

- [ ] Implement `TaskIntoProjectHandler`.
- [ ] Register.
- [ ] Colocate tests.

## Acceptance Criteria

- [ ] Handler matches `("task", "project")`.
- [ ] Pasting a task onto a project creates a new task with `project: <id>` set.
- [ ] Source task's `column` is preserved if set; if not set, falls back gracefully.
- [ ] Cut variant deletes the source task.

## Tests

- [ ] `paste_task_into_project_sets_project_field` — assert the new task's `project` equals the target id.
- [ ] `paste_task_into_project_preserves_source_column` — source had `column: doing`; new task also has `column: doing`.
- [ ] `paste_cut_task_into_project_deletes_source`.
- [ ] Run command: `cargo nextest run -p swissarmyhammer-kanban paste_handlers::task_into_project` — all green.

## Workflow

- Use `/tdd`.

#commands

Depends on: 01KPG5YB7GTQ6Q3CEQAMXPJ58F (mechanism)