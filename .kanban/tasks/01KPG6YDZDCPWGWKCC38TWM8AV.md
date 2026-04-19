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
position_column: done
position_ordinal: fffffffffffffffffffffff880
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

- [x] Delete the 2 cross-cutting opt-ins. (Note: `ui.inspect` was already absent from board.yaml; only `entity.paste` was present and is now removed. The `commands:` key was removed entirely.)
- [x] Hygiene test green for board.yaml. (board.yaml no longer appears in `yaml_hygiene_no_cross_cutting_in_entity_schemas` violations.)

## Acceptance Criteria

- [x] `board.yaml`'s `commands:` is empty or absent. (Removed entirely.)
- [x] Right-click on a board background shows Inspect Board, Paste (when task or column is on the clipboard). (Verified by `entity_paste_surfaces_on_board_when_task_clipboard` test — paste surfaces via the global registry pass with `available: true` when task clipboard + board scope.)
- [x] Hygiene test green for board.yaml. (board.yaml dropped from violations.)

## Tests

- [x] Add `entity_paste_surfaces_on_board_when_task_clipboard` — scope `["board:main"]` with task clipboard; emission contains `entity.paste` with `available: true`. (Added at scope_commands.rs around the board scope test cluster.)
- [x] Run command: `cargo nextest run -p swissarmyhammer-kanban scope_commands board` — all green.

## Implementation Notes

- The cross-cutting auto-emit pass (`emit_cross_cutting_commands`) keys off `from: target` on the first param. `entity.paste` declares `from: scope_chain` (column param), so it does NOT auto-emit per moniker — it surfaces via the global registry pass with `target: None` and `PasteCmd::available()` gates it against scope+clipboard. The new test pins this contract.
- The pre-existing failing test `entity_delete_surfaces_on_project_via_autoemit` is part of the parallel project.yaml cleanup task (different ticket).

## Workflow

- Use `/tdd`.

#commands

Depends on: 01KPG6W9GCCRNZC81C4Z92QTNA, 01KPEMYJV7BMTJB6GZ8MGTD04J, 01KPG5XK61ND4JKXW3FCM3CC97, 01KPG5YB7GTQ6Q3CEQAMXPJ58F, 01KPG6GD34NMPQE1DZD0MHWE0N (task→board), 01KPG6H74Z24N48DQR75CT7HP7 (column→board)