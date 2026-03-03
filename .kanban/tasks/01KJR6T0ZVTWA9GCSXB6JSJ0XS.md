---
title: Deprecate legacy read_task/write_task methods
position:
  column: todo
  ordinal: c2
---
**context.rs lines 371-477**

Tasks can be read/written through two independent I/O paths: the legacy typed path (`ctx.read_task()/write_task()`) and the new entity path (`ectx.read("task", id)/write(&entity)`). Both operate on the same `.md` files but use different field schemas (nested `position` struct vs flat `position_column`). If both paths read the same file, they interpret/write it differently.

Remaining legacy callers: `column/delete.rs`, `swimlane/delete.rs`, `defaults.rs::KanbanLookup`, and context tests.

**Done:** Added `#[deprecated]` annotations to `read_task()`, `write_task()`, `read_all_tasks()` with `#[allow(deprecated)]` on known legacy callers.

**Still needed** (in next migration wave):
- [ ] Migrate column/delete.rs to entity path
- [ ] Migrate swimlane/delete.rs to entity path
- [ ] Migrate defaults.rs::KanbanLookup to entity path
- [ ] Update context tests
- [ ] **Remove migration code** (migrate_storage, JSON-to-YAML migration in read_task, etc.) — user says drop it as we go
- [ ] Verify tests pass"