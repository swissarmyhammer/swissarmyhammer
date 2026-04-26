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
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffa80
title: 'Commands: task.yaml cleanup — move task/attachment declarations in, purge cross-cutting opt-ins'
---
## What

Finish the task entity's command story. Per the final architecture: type-specific commands live as full `CommandDef` entries in `swissarmyhammer-commands/builtin/commands/<noun>.yaml`. Entity schemas carry **no** `commands:` list.

### Create `builtin/commands/task.yaml`

NEW file `swissarmyhammer-commands/builtin/commands/task.yaml` holding full `CommandDef` entries for the task-specific commands currently squatting in `entity.yaml`:

- `task.move`
- `task.delete`
- `task.untag`
- `task.doThisNext`

Copy each entry's `id`, `name`, `scope`, `undoable`, `context_menu`, `keys`, `params` verbatim from `entity.yaml` (no semantics change).

### `attachment.delete` goes into `builtin/commands/attachment.yaml`

`attachment.yaml` already exists in `builtin/commands/` holding `attachment.open` and `attachment.reveal`. Append `attachment.delete` as a full `CommandDef` (id, name, scope, undoable, visible, params). Remove from `entity.yaml`.

### Remove from `entity.yaml`

Delete the 5 entries moved above (`task.move`, `task.delete`, `task.untag`, `task.doThisNext`, `attachment.delete`). Rely on the header comment left by card X to explain why they now live elsewhere.

### Remove `commands:` from task entity schema

`swissarmyhammer-kanban/builtin/entities/task.yaml` — delete the entire `commands:` list. The entity schema describes fields only from here on.

### Files to touch

- CREATE `swissarmyhammer-commands/builtin/commands/task.yaml`
- MODIFY `swissarmyhammer-commands/builtin/commands/attachment.yaml` — append `attachment.delete`
- MODIFY `swissarmyhammer-commands/builtin/commands/entity.yaml` — remove 5 task.*/attachment.delete entries
- MODIFY `swissarmyhammer-kanban/builtin/entities/task.yaml` — delete `commands:` list entirely

### Subtasks

- [ ] Create `task.yaml` command file with the 4 task.* declarations.
- [ ] Append `attachment.delete` to `attachment.yaml` command file.
- [ ] Delete the 5 moved entries from `entity.yaml`.
- [ ] Delete `commands:` list from `task.yaml` entity schema.
- [ ] `yaml_hygiene_no_cross_cutting_in_entity_schemas` — zero task.yaml findings.

## Acceptance Criteria

- [ ] `task.yaml`'s entity schema has no `commands:` key.
- [ ] `entity.yaml` no longer contains `task.*` or `attachment.delete`.
- [ ] `builtin/commands/task.yaml` exists and declares `task.move`, `task.delete`, `task.untag`, `task.doThisNext`.
- [ ] `builtin/commands/attachment.yaml` declares `attachment.delete` in addition to the existing open/reveal.
- [ ] Right-click on a task still shows Inspect / Copy / Cut / Paste / Archive / Unarchive / Delete / Move / Do This Next / Remove Tag with correct keybindings (surface via cross-cutting + type-specific registry emission).
- [ ] Hygiene test is green for task.yaml.

## Tests

- [ ] Existing task-scope emission tests still pass.
- [ ] `test_all_yaml_commands_have_rust_implementations` passes (command declarations moved but impls unchanged).
- [ ] Run command: `cargo nextest run -p swissarmyhammer-kanban -p swissarmyhammer-commands` — all green.

## Workflow

- Use `/tdd` — the hygiene test drives this card.

#commands

Depends on: 01KPG6W9GCCRNZC81C4Z92QTNA (entity.yaml cross-cutting home), 01KPEMYJV7BMTJB6GZ8MGTD04J (auto-emit mechanism), 01KPG5XK61ND4JKXW3FCM3CC97 (copy/cut generalized), 01KPG5YB7GTQ6Q3CEQAMXPJ58F (paste mechanism), 01KPG6G4SGXW1FN92YDXFNEAQ2 (task→column), 01KPG6GD34NMPQE1DZD0MHWE0N (task→board), 01KPG6GN9JQSCZKFER5ZJ5JC62 (task→project), 01KPG6GYSNGTEJ42XA2QNB3VE0 (tag→task)