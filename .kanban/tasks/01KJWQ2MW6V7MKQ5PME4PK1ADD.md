---
position_column: done
position_ordinal: k5
title: 'Eliminate primitive obsession: newtypes for EntityId, EntityType, FieldName, ChangeEntryId, TransactionId'
---
**Review finding: Warning (cross-crate)**

Previous agent completed the newtypes but put the `define_id!` macro in `swissarmyhammer-fields`. User specified it belongs in `swissarmyhammer-common`. Need to move it.

## Remaining work
- [ ] Move `define_id!` macro from swissarmyhammer-fields/src/id_types.rs to swissarmyhammer-common
- [ ] Have swissarmyhammer-fields re-export from common instead of defining it
- [ ] Verify all crates compile and tests pass"