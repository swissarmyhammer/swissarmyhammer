---
assignees:
- claude-code
depends_on:
- 01KPG6W9GCCRNZC81C4Z92QTNA
- 01KPEMYJV7BMTJB6GZ8MGTD04J
- 01KPG5XK61ND4JKXW3FCM3CC97
- 01KPG5YB7GTQ6Q3CEQAMXPJ58F
- 01KPG6GD34NMPQE1DZD0MHWE0N
- 01KPG6H74Z24N48DQR75CT7HP7
position_column: todo
position_ordinal: ea80
title: 'Commands: board.yaml cleanup — purge cross-cutting opt-ins'
---
## What

Clean up `swissarmyhammer-kanban/builtin/entities/board.yaml`: no type-specific commands; just purge cross-cutting opt-ins.

### Moves IN

None.

### Moves OUT

Delete these:

- `ui.inspect`
- `entity.paste`

### Files to touch

- `swissarmyhammer-kanban/builtin/entities/board.yaml` — empty the `commands:` list (or remove the key).

### Subtasks

- [ ] Delete the 2 cross-cutting opt-ins.
- [ ] Hygiene test green for board.yaml.

## Acceptance Criteria

- [ ] `board.yaml`'s `commands:` is empty or absent.
- [ ] Right-click on a board background shows Inspect Board, Paste (when task or column is on the clipboard).
- [ ] Hygiene test green for board.yaml.

## Tests

- [ ] Add `entity_paste_surfaces_on_board_when_task_clipboard` — scope `["board:main"]` with task clipboard; emission contains `entity.paste` with `available: true`.
- [ ] Run command: `cargo nextest run -p swissarmyhammer-kanban scope_commands board` — all green.

## Workflow

- Use `/tdd`.

#commands

Depends on: 01KPG6W9GCCRNZC81C4Z92QTNA, 01KPEMYJV7BMTJB6GZ8MGTD04J, 01KPG5XK61ND4JKXW3FCM3CC97, 01KPG5YB7GTQ6Q3CEQAMXPJ58F, 01KPG6GD34NMPQE1DZD0MHWE0N (task→board), 01KPG6H74Z24N48DQR75CT7HP7 (column→board)