---
assignees:
- claude-code
depends_on:
- 01KPG6W9GCCRNZC81C4Z92QTNA
- 01KPEMYJV7BMTJB6GZ8MGTD04J
- 01KPG5XK61ND4JKXW3FCM3CC97
- 01KPG5YB7GTQ6Q3CEQAMXPJ58F
- 01KPG6GYSNGTEJ42XA2QNB3VE0
position_column: todo
position_ordinal: f580
title: 'Commands: tag.yaml cleanup — move tag.update declaration in, purge cross-cutting opt-ins'
---
## What

Finish the tag entity's command story per the final architecture: type-specific `tag.update` lives as a full `CommandDef` in `swissarmyhammer-commands/builtin/commands/tag.yaml` (NEW file). The tag entity schema loses its `commands:` list entirely.

### Create `builtin/commands/tag.yaml`

NEW file `swissarmyhammer-commands/builtin/commands/tag.yaml` holding the full `tag.update` declaration — copy verbatim from `entity.yaml` (id, name, scope, undoable, visible, params).

### Remove from `entity.yaml`

Delete the `tag.update` entry.

### Remove `commands:` from tag entity schema

`swissarmyhammer-kanban/builtin/entities/tag.yaml` — delete the entire `commands:` list. Entity schema is fields-only.

### Files to touch

- CREATE `swissarmyhammer-commands/builtin/commands/tag.yaml`
- MODIFY `swissarmyhammer-commands/builtin/commands/entity.yaml` — remove `tag.update`
- MODIFY `swissarmyhammer-kanban/builtin/entities/tag.yaml` — delete `commands:` list

### Subtasks

- [ ] Create `tag.yaml` command file with `tag.update`.
- [ ] Delete `tag.update` from `entity.yaml`.
- [ ] Delete `commands:` list from `tag.yaml` entity schema.
- [ ] Hygiene test green for tag.yaml.

## Acceptance Criteria

- [ ] `tag.yaml`'s entity schema has no `commands:` key.
- [ ] `entity.yaml` no longer contains `tag.update`.
- [ ] `builtin/commands/tag.yaml` exists and declares `tag.update`.
- [ ] Right-click on a tag shows Inspect Tag, Delete Tag (via auto-emit entity.delete), Archive Tag, Copy Tag, Cut Tag.
- [ ] Hygiene test green for tag.yaml.

## Tests

- [ ] Existing tag-scope emission tests still pass.
- [ ] `test_all_yaml_commands_have_rust_implementations` passes.
- [ ] Run command: `cargo nextest run -p swissarmyhammer-kanban scope_commands tag` — all green.

## Workflow

- Use `/tdd` — hygiene test drives this card.

#commands

Depends on: 01KPG6W9GCCRNZC81C4Z92QTNA, 01KPEMYJV7BMTJB6GZ8MGTD04J, 01KPG5XK61ND4JKXW3FCM3CC97, 01KPG5YB7GTQ6Q3CEQAMXPJ58F, 01KPG6GYSNGTEJ42XA2QNB3VE0 (tag→task paste handler)