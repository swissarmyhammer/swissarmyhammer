---
assignees:
- assistant
- wballard
position_column: done
position_ordinal: a180
title: Complete FieldDefId newtype — eliminate bare Ulid in fields crate
---
@Will Ballard

FieldDef.id is bare `ulid::Ulid`. FieldsContext uses `HashMap<Ulid, usize>` for id_index. All methods that accept or return field IDs use bare Ulid. Need a FieldDefId newtype using define_id! macro.

~30 bare Ulid usages across types.rs, context.rs, compute.rs, validation.rs plus downstream in kanban defaults.rs.

- [x] Add `FieldDefId` to swissarmyhammer-fields/src/id_types.rs via define_id!
- [ ] Change FieldDef.id from Ulid to FieldDefId
- [ ] Update FieldsContext id_index and all methods
- [x] Update all test code constructing FieldDef
- [ ] Update kanban defaults.rs
- [ ] Re-export from lib.rs
- [ ] Run full test suite