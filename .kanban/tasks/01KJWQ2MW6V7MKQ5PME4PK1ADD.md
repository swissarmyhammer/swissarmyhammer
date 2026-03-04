---
position_column: todo
position_ordinal: d6
title: 'Eliminate primitive obsession: newtypes for EntityId, EntityType, FieldName, ChangeEntryId, TransactionId'
---
**Review finding: Warning (cross-crate)**

The `define_id!` macro already exists in `swissarmyhammer-kanban/src/types/ids.rs` and works well for TaskId, TagId, ColumnId, etc. But the entity and fields crates pass entity IDs, entity types, field names, change entry ULIDs, and transaction IDs as bare `&str` / `String` everywhere. ~60+ bare string parameters across the two crates.

This makes it possible to accidentally pass an entity ID where an entity type is expected, or a transaction ID where a change entry ID is expected, with no compile-time protection.

## Scope

### New newtypes needed (at minimum)
- `EntityId` — entity instance IDs (ULIDs or slugs)
- `EntityTypeName` — entity type names ("task", "tag", "column")
- `FieldName` — field names ("title", "status", "body")
- `ChangeEntryId` — changelog entry ULIDs
- `TransactionId` — transaction ULIDs

### Where to define them
The `define_id!` macro currently lives in the kanban crate. It should move to a shared location — either swissarmyhammer-entity or a new tiny types crate. The kanban crate's existing types (TaskId, TagId, etc.) would then build on top.

### Affected files (~60+ parameter changes)
**swissarmyhammer-entity:**
- context.rs — all public methods (read, write, delete, undo, redo, etc.)
- changelog.rs — ChangeEntry struct fields and constructors
- entity.rs — Entity struct fields
- io.rs — read/write/trash functions
- error.rs — error variant fields

**swissarmyhammer-fields:**
- types.rs — FieldDef.name, EntityDef.name, FieldType::Reference { entity }
- context.rs — lookup methods, indexes
- validation.rs — EntityLookup trait
- compute.rs — derivation keys

## Checklist
- [ ] Move `define_id!` macro to swissarmyhammer-entity (or shared crate)
- [ ] Define EntityId, EntityTypeName, FieldName, ChangeEntryId, TransactionId
- [ ] Update Entity struct (entity_type, id fields)
- [ ] Update EntityContext public API
- [ ] Update ChangeEntry struct and constructors
- [ ] Update io.rs functions
- [ ] Update FieldDef and EntityDef in fields crate
- [ ] Update EntityLookup trait
- [ ] Update kanban crate callers
- [ ] Run full test suite