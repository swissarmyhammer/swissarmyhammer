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
position_column: done
position_ordinal: fffffffffffffffffffffffa80
title: 'Commands: column.yaml cleanup — move column.reorder declaration in, purge cross-cutting opt-ins'
---
## What

Finish the column entity's command story. The implementor has already created `swissarmyhammer-commands/builtin/commands/column.yaml` with `column.reorder` as a full `CommandDef` (landed in WIP). This card completes the migration by removing the duplicate entries from both `entity.yaml` and the column entity schema.

### Remove from `entity.yaml`

Delete the `column.reorder` entry — it now lives in `builtin/commands/column.yaml`.

### Remove `commands:` from column entity schema

`swissarmyhammer-kanban/builtin/entities/column.yaml` — delete the entire `commands:` list. Entity schema is fields-only.

### Files to touch

- MODIFY `swissarmyhammer-commands/builtin/commands/entity.yaml` — remove `column.reorder`
- MODIFY `swissarmyhammer-kanban/builtin/entities/column.yaml` — delete `commands:` list

### Subtasks

- [ ] Verify `builtin/commands/column.yaml` already declares `column.reorder` (landed via WIP).
- [ ] Delete `column.reorder` from `entity.yaml`.
- [ ] Delete `commands:` list from column entity schema.
- [ ] Hygiene test green for column.yaml.

## Acceptance Criteria

- [ ] `column.yaml`'s entity schema has no `commands:` key.
- [ ] `entity.yaml` no longer contains `column.reorder`.
- [ ] `builtin/commands/column.yaml` declares `column.reorder` (verify).
- [ ] Right-click on a column shows Inspect Column, Delete Column (auto-emit), Paste (when clipboard applicable).
- [ ] Hygiene test green for column.yaml.

## Tests

- [ ] Existing column-scope emission tests still pass.
- [ ] Run command: `cargo nextest run -p swissarmyhammer-kanban scope_commands column` — all green.

## Workflow

- Use `/tdd`.

#commands

Depends on: 01KPG6W9GCCRNZC81C4Z92QTNA, 01KPEMYJV7BMTJB6GZ8MGTD04J, 01KPG5XK61ND4JKXW3FCM3CC97, 01KPG5YB7GTQ6Q3CEQAMXPJ58F, 01KPG6G4SGXW1FN92YDXFNEAQ2 (task→column), 01KPG6H74Z24N48DQR75CT7HP7 (column→board)