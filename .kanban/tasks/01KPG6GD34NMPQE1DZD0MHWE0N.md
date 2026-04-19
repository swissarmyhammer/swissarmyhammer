---
assignees:
- claude-code
depends_on:
- 01KPG5YB7GTQ6Q3CEQAMXPJ58F
position_column: done
position_ordinal: fffffffffffffffffffffff080
title: 'Commands: paste handler — task onto board'
---
## What

Implement `TaskIntoBoardHandler` — pastes a task into the *leftmost column* of a target board when no specific column is in the scope chain. Handler at `swissarmyhammer-kanban/src/commands/paste_handlers/task_into_board.rs`, matches `("task", "board")`.

### Action

1. Parse board id from the target moniker.
2. Query the board's columns via `EntityContext`, order by position, pick the leftmost (index 0).
3. Error if the board has no columns (`available()` can also gate this if cheap).
4. Clone `clipboard.fields`; set `column` to the leftmost column id; drop stale `ordinal`.
5. Invoke `AddEntity::new("task").with_overrides(fields)`.
6. If `clipboard.is_cut`, delete the source task.

### Files

- CREATE `swissarmyhammer-kanban/src/commands/paste_handlers/task_into_board.rs`.
- MODIFY `swissarmyhammer-kanban/src/commands/paste_handlers/mod.rs` — register call.

### Subtasks

- [x] Implement leftmost-column lookup helper (reuse existing positional query if one exists).
- [x] Implement `TaskIntoBoardHandler`.
- [ ] Register. (Deferred — orchestrator will batch-register `m.register(TaskIntoBoardHandler);` to avoid mod.rs collision with sibling agents implementing other paste handlers.)
- [x] Colocate tests.

## Acceptance Criteria

- [x] Handler matches `("task", "board")`.
- [x] Pasting a task onto a board creates a new task in the leftmost column.
- [x] When the board has no columns, `available()` returns false (no crash, no silent no-op).
- [x] Cut variant deletes the source task.

## Tests

- [x] `paste_task_into_board_uses_leftmost_column` — fixture with columns at positions 0, 100, 200; assert new task lands in position-0 column.
- [x] `paste_task_into_empty_board_unavailable` — board with zero columns; handler's `available()` returns false.
- [x] `paste_cut_task_into_board_deletes_source` — cut path.
- [x] Run command: `cargo nextest run -p swissarmyhammer-kanban paste_handlers::task_into_board` — all green (8 tests pass).

## Workflow

- Use `/tdd` — write `paste_task_into_board_uses_leftmost_column` first.

#commands

Depends on: 01KPG5YB7GTQ6Q3CEQAMXPJ58F (mechanism)