---
position_column: done
position_ordinal: i0
title: Implement EntityContext::undo(ulid)
---
Add an `undo` method to EntityContext that reverses a specific changelog operation by its ULID. EntityContext::undo() handles both single-entity ops and transactions (finds all constituent entries by transaction_id and undoes them in reverse order).

## How it works

Given a ULID, undo needs to:
1. Look up (entity_type, entity_id) from the in-memory index
2. Read the changelog (falls back to trash dir for deleted entities)
3. Find the ChangeEntry by ULID
4. Apply `reverse_changes()` to get the inverse
5. Write the entity back, logging an "undo" changelog entry referencing the original ULID

## Three cases

### Undo of update (op: "update")
- Read entity, apply reversed changes, write entity back
- Straightforward — reverse_changes + apply_changes already work

### Undo of create (op: "create")
- The entity was just created — undoing it means deleting it
- All fields in the ChangeEntry are `Set { value }` — reversing gives `Removed { old_value }`
- Call `EntityContext::delete()` to move to trash
- Log an "undo" entry before trashing

### Undo of delete (op: "delete")
- The entity was deleted — undoing it means restoring from trash
- read_changelog() falls back to trash dir to find the entry
- Move files back from trash to live storage
- All fields in the ChangeEntry are `Removed { old_value }` — reversing gives `Set { value }`
- Need a new `EntityContext::restore_from_trash()` helper

## Stale undo policy
If the entity has been modified since the operation being undone, TextDiff patches may not apply cleanly — this is a **hard error**. Return an error, no silent data corruption. The client removes the failed entry from its undo stack.

## Design decisions
- The undo operation itself logs a new ChangeEntry with `op: "undo"` and an `undone_id` field referencing the original operation ULID
- undo returns the new ChangeEntry ULID
- EntityContext::undo() also handles transaction ULIDs — checks if any ChangeEntries have a matching `transaction_id`, collects them all, and undoes in reverse order

## Files
- `swissarmyhammer-entity/src/context.rs` — add `undo()` method, add `restore_from_trash()` helper
- `swissarmyhammer-entity/src/changelog.rs` — add `undone_id` / `redone_id` optional fields to ChangeEntry

## Checklist
- [ ] Add `restore_from_trash()` to EntityContext (moves files back from trash dir)
- [ ] Add `undone_id` / `redone_id` optional fields to ChangeEntry for audit trail
- [ ] Implement `EntityContext::undo()` — handle update, create, delete cases
- [ ] Undo of update: read entity, apply reversed changes, write back
- [ ] Undo of create: delete the entity
- [ ] Undo of delete: restore from trash
- [ ] Stale undo returns hard error (TextDiff patch doesn't apply)
- [ ] Write tests for each case
- [ ] Run full test suite