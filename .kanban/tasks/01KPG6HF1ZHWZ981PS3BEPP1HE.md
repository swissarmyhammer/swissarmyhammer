---
assignees:
- claude-code
depends_on:
- 01KPG5YB7GTQ6Q3CEQAMXPJ58F
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffeb80
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

- [x] Audit existing assignee ops; use the one that appends an assignee. — Used `crate::task::AssignTask` (already idempotent: only appends an assignee not already present).
- [x] Implement `ActorOntoTaskHandler`.
- [x] Register. `m.register(ActorOntoTaskHandler);` added to `paste_handlers/mod.rs::register_paste_handlers()` during the orchestrator's batch-registration after all 7 sibling handler files landed.
- [x] Colocate tests.

## Acceptance Criteria

- [x] Handler matches `("actor", "task")`.
- [x] Pasting an actor onto a task adds them to the task's assignees.
- [x] `is_cut` is ignored — source actor entity not deleted.
- [x] Re-pasting the same actor is idempotent.

## Tests

- [x] `paste_actor_onto_task_adds_assignee` — task has no assignees; after paste, has actor.
- [x] `paste_actor_onto_task_ignores_cut_flag`.
- [x] `paste_same_actor_twice_is_idempotent`.
- [x] Bonus: `handler_matches_actor_onto_task` (dispatch key + matrix lookup) and `paste_actor_onto_non_task_target_errors` (clear failure on wrong target type).
- [x] Run command: `cargo nextest run -p swissarmyhammer-kanban paste_handlers::actor_onto_task` — 5/5 pass after sibling handlers and batch registration landed.

## Workflow

- Use `/tdd`.

#commands

Depends on: 01KPG5YB7GTQ6Q3CEQAMXPJ58F (mechanism)