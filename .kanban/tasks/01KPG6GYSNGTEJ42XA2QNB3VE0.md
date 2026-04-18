---
assignees:
- claude-code
depends_on:
- 01KPG5YB7GTQ6Q3CEQAMXPJ58F
position_column: todo
position_ordinal: e180
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
- MODIFY `swissarmyhammer-kanban/src/commands/paste_handlers/mod.rs` — register call.

### Subtasks

- [ ] Audit existing tag-task association op; use it or create one if missing.
- [ ] Implement `TagOntoTaskHandler`.
- [ ] Register.
- [ ] Colocate tests.

## Acceptance Criteria

- [ ] Handler matches `("tag", "task")`.
- [ ] Pasting a tag onto a task adds the tag to the task's tag list.
- [ ] `is_cut` is ignored — source tag entity not deleted.
- [ ] Re-pasting the same tag on the same task is idempotent.

## Tests

- [ ] `paste_tag_onto_task_adds_tag` — task has no tags initially; after paste, task has the tag.
- [ ] `paste_tag_onto_task_ignores_cut_flag` — cut tag still exists after paste.
- [ ] `paste_same_tag_twice_is_idempotent` — paste twice; task has tag exactly once.
- [ ] Run command: `cargo nextest run -p swissarmyhammer-kanban paste_handlers::tag_onto_task` — all green.

## Workflow

- Use `/tdd` — `paste_tag_onto_task_adds_tag` first.

#commands

Depends on: 01KPG5YB7GTQ6Q3CEQAMXPJ58F (mechanism)