---
title: Default columns defined in three separate places
position:
  column: todo
  ordinal: d1
---
**File:** Multiple files\n\n**What:** The default column definitions (todo/doing/done) are duplicated in three places:\n1. `types/board.rs` -- `Board::default_columns()` returns `Vec<Column>` (dead code after migration)\n2. `board/init.rs` -- `default_columns()` returns `Vec<(&str, &str, usize)>` tuples\n3. `processor.rs` -- inline array literal `[(\"todo\", \"To Do\", 0), ...]` in auto-init logic\n\n**Why it matters:** If a developer changes the default columns in one place but not the others, the board initialization (explicit via `init board`) and auto-initialization (via processor) would produce different column sets. The `Board::default_columns()` method is also dead code since InitBoard no longer calls it.\n\n**Suggestion:**\n- [ ] Remove `Board::default_columns()` from `types/board.rs` (dead code)\n- [ ] Extract a single source of truth for default columns (e.g. a const or function in `defaults.rs`)\n- [ ] Have both `InitBoard` and `KanbanOperationProcessor` reference the same definition\n- [ ] Verify with tests\n\n#warning #warning