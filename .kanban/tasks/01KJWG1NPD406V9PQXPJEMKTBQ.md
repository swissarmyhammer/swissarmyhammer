---
position_column: done
position_ordinal: i3
title: Transaction support in kanban processor for compound undo/redo
---
Composite kanban operations (DeleteTag, MoveTask, AddTask with auto-tag-creation, UpdateTag with bulk rename) touch multiple entities. The client needs a single ULID to undo/redo the whole compound operation as one unit.

## Design

The kanban processor wraps composite operations in a transaction:
1. Operation starts — processor generates a transaction ULID
2. Each entity-level write/delete during the operation records the transaction ULID on its ChangeEntry
3. Operation completes — processor returns the transaction ULID to the caller

### ChangeEntry addition
- Add optional `transaction_id: Option<String>` field to ChangeEntry
- When set, this entry is part of a larger transaction

### EntityContext changes
- Add a way to set a "current transaction ID" that gets stamped on all ChangeEntries during write/delete
- Could be a method like `with_transaction(tx_id)` that returns a scoped context, or a `set_transaction()` / `clear_transaction()` pair

### Kanban processor changes
- Generate transaction ULID before executing composite operations
- Pass it through to EntityContext so all writes get stamped
- Return transaction ULID in the operation result

### Transaction undo/redo
- `undo(transaction_ulid)` — find all ChangeEntries with that transaction_id, apply all their inverses in reverse order
- `redo(transaction_ulid)` — find all ChangeEntries with that transaction_id, apply all their forwards in original order
- This could live on EntityContext or as a new method on KanbanContext

### Finding entries by transaction_id
- Need to scan changelogs to find entries with matching transaction_id
- Could maintain a lightweight in-memory index, or scan the global activity log
- The activity log already records all operations — could add transaction_id there too

## Files
- `swissarmyhammer-entity/src/changelog.rs` — add transaction_id to ChangeEntry
- `swissarmyhammer-entity/src/context.rs` — transaction scoping on EntityContext
- `swissarmyhammer-kanban/src/processor.rs` — generate and pass transaction ULIDs

## Checklist
- [ ] Add transaction_id field to ChangeEntry
- [ ] Add transaction scoping to EntityContext
- [ ] Update KanbanOperationProcessor to generate transaction ULIDs for composite ops
- [ ] Implement transaction-level undo (reverse all constituent ops)
- [ ] Implement transaction-level redo (forward all constituent ops)
- [ ] Test: DeleteTag (touches tag + N tasks) undo restores tag and all task bodies
- [ ] Test: MoveTask with auto-column-creation undo removes column and moves task back
- [ ] Test: AddTask with auto-tag-creation undo removes task and auto-created tags
- [ ] Run full test suite