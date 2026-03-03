---
title: Remove directory creation side-effect from init_entity_context()
position:
  column: done
  ordinal: d5
---
**context.rs lines 1005-1009 and 1012-1044**

The `entity_context()` lazy init calls `init_entity_context()` which calls `fs::create_dir_all()` for the fields directory structure. This means the first call to `entity_context()` — even a read-only operation like `list_entities_generic()` — will create directories on disk.

Read operations should not have write side-effects. If `.kanban` doesn't exist, creating `fields/definitions/` subdirs is surprising.

**Suggestion:** Move `create_dir_all` calls to `open()` or `ensure_directories()` where dir creation is explicit intent. In `init_entity_context()`, use `load_yaml_dir()` with best-effort semantics (it already returns empty vec for missing dirs).

- [ ] Move `create_dir_all` for fields dirs out of `init_entity_context()`
- [ ] Add fields dir creation to `ensure_directories()` 
- [ ] Verify `init_entity_context()` works with missing dirs (returns empty field set)
- [ ] Verify tests pass #warning