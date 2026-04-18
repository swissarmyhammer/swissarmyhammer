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
position_ordinal: e780
title: 'Commands: tag.yaml cleanup — move tag.update declaration in, purge cross-cutting opt-ins'
---
## What

Clean up `swissarmyhammer-kanban/builtin/entities/tag.yaml`: type-specific `tag.update` lives here as a full declaration; cross-cutting commands auto-emit and are not mentioned.

### Moves IN

- Move `tag.update` declaration from `swissarmyhammer-commands/builtin/commands/entity.yaml` into `tag.yaml`'s `commands:` list. Full entry with `name`, `visible: false`, `params`.

### Moves OUT

Delete these from tag.yaml (they auto-emit now):

- `ui.inspect`
- `entity.archive`
- `entity.copy`
- `entity.cut`

### Files to touch

- `swissarmyhammer-kanban/builtin/entities/tag.yaml`
- `swissarmyhammer-commands/builtin/commands/entity.yaml` — remove `tag.update`.

### Subtasks

- [ ] Move `tag.update` declaration from entity.yaml to tag.yaml.
- [ ] Delete the 4 cross-cutting opt-ins from tag.yaml.
- [ ] Hygiene test green for tag.yaml.

## Acceptance Criteria

- [ ] `tag.yaml`'s `commands:` list contains exactly `tag.update`.
- [ ] `entity.yaml` no longer contains `tag.update`.
- [ ] Right-click on a tag shows Inspect Tag, Delete Tag (via auto-emit entity.delete), Archive Tag, Copy Tag, Cut Tag.
- [ ] Hygiene test green for tag.yaml.

## Tests

- [ ] Existing tag-scope emission tests still pass.
- [ ] Run command: `cargo nextest run -p swissarmyhammer-kanban scope_commands tag` — all green.

## Workflow

- Use `/tdd` — hygiene test drives this card.

#commands

Depends on: 01KPG6W9GCCRNZC81C4Z92QTNA, 01KPEMYJV7BMTJB6GZ8MGTD04J, 01KPG5XK61ND4JKXW3FCM3CC97, 01KPG5YB7GTQ6Q3CEQAMXPJ58F, 01KPG6GYSNGTEJ42XA2QNB3VE0 (tag→task paste handler)