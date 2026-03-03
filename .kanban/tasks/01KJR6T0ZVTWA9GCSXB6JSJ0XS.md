---
position_column: done
position_ordinal: f4
title: Deprecate legacy read_task/write_task methods
---
**Done.** All legacy task I/O methods removed and callers migrated.\n\n- [x] Migrate column/delete.rs — already on entity path\n- [x] Migrate swimlane/delete.rs — already on entity path\n- [x] Migrate defaults.rs::KanbanLookup to entity path\n- [x] Update context tests (test_task_io rewritten)\n- [x] Remove migration code (read_task, write_task, read_all_tasks, delete_task_file, list_task_ids, TaskMeta, parse_task_markdown)\n- [x] Verify tests pass (218 unit + 7 integration + 1 doc-test, clippy clean)