---
title: Add column/swimlane file I/O to KanbanContext
position:
  column: done
  ordinal: a6
---
Add file-based storage primitives for columns and swimlanes to KanbanContext, following the same pattern as actors and tags (individual JSON files in dedicated directories).

Changes to context.rs:
- Add `columns_dir()` → `.kanban/columns/`
- Add `column_path(id)` → `.kanban/columns/{id}.json`
- Add `column_log_path(id)` → `.kanban/columns/{id}.jsonl`
- Add `read_column(id)`, `write_column(column)`, `delete_column_file(id)`
- Add `list_column_ids()`, `read_all_columns()`
- Same set for swimlanes: `swimlanes_dir()`, `swimlane_path()`, etc.
- Update `create_directories()` and `directories_exist()` to include columns/ and swimlanes/ dirs

This is purely additive - no existing code changes, just new methods on KanbanContext.

Files: swissarmyhammer-kanban/src/context.rs
Verify: cargo test -p swissarmyhammer-kanban passes, new I/O methods have unit tests