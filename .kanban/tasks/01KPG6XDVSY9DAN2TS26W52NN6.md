---
assignees:
- claude-code
depends_on:
- 01KPG6W9GCCRNZC81C4Z92QTNA
- 01KPEMYJV7BMTJB6GZ8MGTD04J
- 01KPG5XK61ND4JKXW3FCM3CC97
- 01KPG5YB7GTQ6Q3CEQAMXPJ58F
- 01KPG6G4SGXW1FN92YDXFNEAQ2
- 01KPG6GD34NMPQE1DZD0MHWE0N
- 01KPG6GN9JQSCZKFER5ZJ5JC62
- 01KPG6GYSNGTEJ42XA2QNB3VE0
position_column: todo
position_ordinal: e680
title: 'Commands: task.yaml cleanup — move task/attachment declarations in, purge cross-cutting opt-ins'
---
## What

Finish `swissarmyhammer-kanban/builtin/entities/task.yaml` so it describes *only* the task entity: its type-specific commands live here as full declarations; cross-cutting commands are not mentioned (they auto-emit from entity.yaml via the scope-chain walker).

### Moves IN (from entity.yaml → task.yaml)

Take these full declarations out of `swissarmyhammer-commands/builtin/commands/entity.yaml` and place them in `task.yaml`'s `commands:` list:

- `task.move`
- `task.delete`
- `task.untag`
- `task.doThisNext`
- `attachment.delete` (attachment operations always occur in task context)

Each becomes a full entry with `name`, `keys`, `context_menu`, `undoable`, and `params`. Remove the same entries from `entity.yaml` in the same commit.

### Moves OUT (purge cross-cutting opt-ins from task.yaml)

Delete these entries entirely — they auto-emit via `emit_cross_cutting_commands` once D / H / I / the paste handlers are in place:

- `ui.inspect`
- `entity.copy`
- `entity.cut`
- `entity.paste`
- `entity.archive`
- `entity.unarchive`

### Files to touch

- `swissarmyhammer-kanban/builtin/entities/task.yaml` — rewrite `commands:` list.
- `swissarmyhammer-commands/builtin/commands/entity.yaml` — remove task.*, attachment.delete entries.

### Subtasks

- [ ] Move the 5 declarations (`task.move`, `task.delete`, `task.untag`, `task.doThisNext`, `attachment.delete`) from entity.yaml into task.yaml with full declarations.
- [ ] Delete the 6 cross-cutting opt-ins (`ui.inspect`, `entity.copy/cut/paste/archive/unarchive`) from task.yaml.
- [ ] Verify the hygiene test `yaml_hygiene_no_cross_cutting_in_entity_schemas` no longer flags task.yaml.

## Acceptance Criteria

- [ ] `task.yaml`'s `commands:` list contains exactly: `task.move`, `task.delete`, `task.untag`, `task.doThisNext`, `attachment.delete`. No `entity.*`, no `ui.inspect`.
- [ ] `entity.yaml` no longer contains `task.*` or `attachment.delete` entries.
- [ ] Right-click on a task still shows Inspect / Copy / Cut / Paste / Archive / Unarchive / Delete / Move / Do This Next / Remove Tag with correct keybindings (emitted via auto-emit + type-specific).
- [ ] Hygiene test from 01KPEM811W5XE6WVHDQVRCZ4B0 is green for task.yaml.

## Tests

- [ ] Existing tests that check task-scope command emission still pass (they're the regression net).
- [ ] Run command: `cargo nextest run -p swissarmyhammer-kanban scope_commands` — all green.

## Workflow

- Use `/tdd` — the hygiene test drives this card; when task.yaml is cleaned, its portion turns green.

#commands

Depends on: 01KPG6W9GCCRNZC81C4Z92QTNA (entity.yaml cross-cutting home), 01KPEMYJV7BMTJB6GZ8MGTD04J (auto-emit mechanism), 01KPG5XK61ND4JKXW3FCM3CC97 (copy/cut generalized), 01KPG5YB7GTQ6Q3CEQAMXPJ58F (paste mechanism), 01KPG6G4SGXW1FN92YDXFNEAQ2 (task→column handler), 01KPG6GD34NMPQE1DZD0MHWE0N (task→board), 01KPG6GN9JQSCZKFER5ZJ5JC62 (task→project), 01KPG6GYSNGTEJ42XA2QNB3VE0 (tag→task)