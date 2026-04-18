---
assignees:
- claude-code
depends_on:
- 01KPG5YB7GTQ6Q3CEQAMXPJ58F
position_column: todo
position_ordinal: e380
title: 'Commands: paste handler — actor onto task'
---
## What

Implement `ActorOntoTaskHandler` — assigns an actor to a task. Handler at `swissarmyhammer-kanban/src/commands/paste_handlers/actor_onto_task.rs`, matches `("actor", "task")`.

### Action

1. Parse task id from target moniker and actor id from `clipboard.entity_id`.
2. Invoke the existing `AssignTask` operation (audit `swissarmyhammer-kanban/src/task/` for the assignee-adding op; likely named `AssignTask` or `AddAssignee`).
3. Ignore `is_cut` — actors are associations, not entities that move.
4. Idempotent — re-pasting the same actor on the same task is a no-op.

### Files

- CREATE `swissarmyhammer-kanban/src/commands/paste_handlers/actor_onto_task.rs`.
- MODIFY `swissarmyhammer-kanban/src/commands/paste_handlers/mod.rs` — register call.

### Subtasks

- [ ] Audit existing assignee ops; use the one that appends an assignee.
- [ ] Implement `ActorOntoTaskHandler`.
- [ ] Register.
- [ ] Colocate tests.

## Acceptance Criteria

- [ ] Handler matches `("actor", "task")`.
- [ ] Pasting an actor onto a task adds them to the task's assignees.
- [ ] `is_cut` is ignored — source actor entity not deleted.
- [ ] Re-pasting the same actor is idempotent.

## Tests

- [ ] `paste_actor_onto_task_adds_assignee` — task has no assignees; after paste, has actor.
- [ ] `paste_actor_onto_task_ignores_cut_flag`.
- [ ] `paste_same_actor_twice_is_idempotent`.
- [ ] Run command: `cargo nextest run -p swissarmyhammer-kanban paste_handlers::actor_onto_task` — all green.

## Workflow

- Use `/tdd`.

#commands

Depends on: 01KPG5YB7GTQ6Q3CEQAMXPJ58F (mechanism)