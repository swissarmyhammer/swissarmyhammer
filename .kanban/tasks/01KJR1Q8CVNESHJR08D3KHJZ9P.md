---
title: EntityContext recreated on every delegation call from KanbanContext
position:
  column: todo
  ordinal: b9
---
**File**: `swissarmyhammer-kanban/src/context.rs`, lines 994-998 and 1002-1028

**What**: Every thin wrapper (`read_entity_generic`, `write_entity_generic`, `delete_entity_generic`, `list_entities_generic`, `read_entity_changelog`) calls `self.entity_context()?` which creates a new `EntityContext` each time. `EntityContext::new()` clones the `PathBuf` root on every invocation.

**Why**: This is a minor performance concern -- `PathBuf::clone()` allocates on every entity operation. More importantly, it is a design smell: if EntityContext ever gains initialization logic (caching, validation), it would run repeatedly. The cost is currently negligible since these are I/O-bound operations, but the pattern does not scale well.

**Suggestion**: Consider caching the EntityContext or storing it as a field alongside `fields`. However, since EntityContext borrows `&FieldsContext`, this creates a self-referential borrow issue. The current approach is pragmatically correct given Rust's borrow rules. Acceptable as-is for now, but worth revisiting if EntityContext grows.

Checklist:
- [ ] Evaluate whether EntityContext should be cached (likely: accept current approach for now)
- [ ] Add a doc comment on `entity_context()` noting the per-call construction is intentional
- [ ] Verify no performance regression with `cargo nextest run` #warning