---
assignees:
- claude-code
depends_on:
- 01KPG6W9GCCRNZC81C4Z92QTNA
- 01KPEMYJV7BMTJB6GZ8MGTD04J
- 01KPG5XK61ND4JKXW3FCM3CC97
- 01KPG5YB7GTQ6Q3CEQAMXPJ58F
- 01KPG6G4SGXW1FN92YDXFNEAQ2
- 01KPG6H74Z24N48DQR75CT7HP7
position_column: todo
position_ordinal: e980
title: 'Commands: column.yaml cleanup — move column.reorder declaration in, purge cross-cutting opt-ins'
---
## What

Clean up `swissarmyhammer-kanban/builtin/entities/column.yaml`: migrate `column.reorder` declaration in; purge cross-cutting opt-ins.

### Moves IN

- `column.reorder` full declaration moves from `entity.yaml` to `column.yaml`.

### Moves OUT

Delete these:

- `ui.inspect`
- `entity.paste`

### Files to touch

- `swissarmyhammer-kanban/builtin/entities/column.yaml`
- `swissarmyhammer-commands/builtin/commands/entity.yaml` — remove `column.reorder`.

### Subtasks

- [ ] Move `column.reorder` from entity.yaml into column.yaml.
- [ ] Delete the 2 cross-cutting opt-ins from column.yaml.
- [ ] Hygiene test green for column.yaml.

## Acceptance Criteria

- [ ] `column.yaml`'s `commands:` list contains exactly `column.reorder`.
- [ ] `entity.yaml` no longer contains `column.reorder`.
- [ ] Right-click on a column shows Inspect Column, Paste into Column (when clipboard holds a task), Delete Column, Reorder (palette only).
- [ ] Hygiene test green for column.yaml.

## Tests

- [ ] Existing column-scope emission tests still pass.
- [ ] Run command: `cargo nextest run -p swissarmyhammer-kanban scope_commands column` — all green.

## Workflow

- Use `/tdd`.

#commands

Depends on: 01KPG6W9GCCRNZC81C4Z92QTNA, 01KPEMYJV7BMTJB6GZ8MGTD04J, 01KPG5XK61ND4JKXW3FCM3CC97, 01KPG5YB7GTQ6Q3CEQAMXPJ58F, 01KPG6G4SGXW1FN92YDXFNEAQ2 (task→column), 01KPG6H74Z24N48DQR75CT7HP7 (column→board)