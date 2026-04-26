---
assignees:
- claude-code
depends_on:
- 01KPG5YB7GTQ6Q3CEQAMXPJ58F
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffea80
title: 'Commands: paste handler — tag onto task'
---
## What

Implement `TagOntoTaskHandler` — adds a tag to a task (tag-the-task; no new entity created). Handler at `swissarmyhammer-kanban/src/commands/paste_handlers/tag_onto_task.rs`, matches `("tag", "task")`.

### Action

1. Parse task id from the target moniker and tag id from `clipboard.entity_id`.
2. Invoke `TagTask` operation (or whatever existing op associates a tag with a task — audit `swissarmyhammer-kanban/src/tag/` or `task/`).
3. Ignore `is_cut` — tags are associations, not entities that move. Cutting a tag and pasting it onto a task should add the association without deleting the tag entity.
4. If the task already has the tag, the op should be idempotent (no-op). Confirm `TagTask` handles this.

### Files

- CREATE `swissarmyhammer-kanban/src/commands/paste_handlers/tag_onto_task.rs`.
- MODIFY `swissarmyhammer-kanban/src/commands/paste_handlers/mod.rs` — register call. **DEFERRED** — orchestrator will batch-register; only the `pub mod tag_onto_task;` line was added (matching sibling pattern) so the file compiles. The actual `m.register(TagOntoTaskHandler);` line is intentionally not added per parallel-safety override.

### Subtasks

- [x] Audit existing tag-task association op; use it or create one if missing. (Used existing `crate::task::TagTask` — handles both slug and ULID resolution and is idempotent via `tag_parser::append_tag`.)
- [x] Implement `TagOntoTaskHandler`.
- [x] Register. **DEFERRED** — orchestrator will batch-register `m.register(TagOntoTaskHandler);`. Only `pub mod tag_onto_task;` was added.
- [x] Colocate tests.

## Acceptance Criteria

- [x] Handler matches `("tag", "task")`.
- [x] Pasting a tag onto a task adds the tag to the task's tag list.
- [x] `is_cut` is ignored — source tag entity not deleted.
- [x] Re-pasting the same tag on the same task is idempotent.

## Tests

- [x] `paste_tag_onto_task_adds_tag` — task has no tags initially; after paste, task has the tag.
- [x] `paste_tag_onto_task_ignores_cut_flag` — cut tag still exists after paste.
- [x] `paste_same_tag_twice_is_idempotent` — paste twice; task has tag exactly once.
- [x] Run command: `cargo nextest run -p swissarmyhammer-kanban paste_handlers::tag_onto_task` — all green (6 tests passing including 3 required + 1 local-matrix sanity + 2 negative-target tests).

## Workflow

- Use `/tdd` — `paste_tag_onto_task_adds_tag` first.

#commands

Depends on: 01KPG5YB7GTQ6Q3CEQAMXPJ58F (mechanism)