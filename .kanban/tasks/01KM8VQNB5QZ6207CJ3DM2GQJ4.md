---
position_column: todo
position_ordinal: 7f80
title: We need to verify that adding a task adds it to the todo column
---
## What

Verify that adding a task (via board.newCard, grid.newBelow, etc.) places it in the first column (todo). This should be tested in the command engine, not via GUI.

### Tests (command layer — no GUI)
- [ ] `cargo nextest run -p swissarmyhammer-kanban` — test that `task.add` command places task in first column
- [ ] Test that `task.add` with explicit column arg places task in that column
- [ ] Test that `task.add` on a board with no columns returns an error
- [ ] Test that `board.newCard` (if it's a separate command) delegates to `task.add` with correct defaults

All tests use `KanbanContext` directly — no Tauri, no windows, no webviews.