---
position_column: done
position_ordinal: i1
title: Implement EntityContext::redo(ulid)
---
Add a `redo` method to EntityContext that re-applies a previously undone operation by its ULID.

## How it works

Redo is the mirror of undo. Given the ULID of the ORIGINAL operation (not the undo entry):
1. Find the ChangeEntry by ULID
2. Read the current entity state
3. Apply the forward changes
4. Write the entity back, logging a "redo" entry referencing the original ULID

## Three cases (mirrors of undo)

### Redo of update
- Read entity, apply forward changes, write back

### Redo of create
- Restore entity from trash (it was deleted by undo-of-create)
- Same restore_from_trash() helper as undo-of-delete

### Redo of delete
- Delete the entity again (it was restored by undo-of-delete)
- Same as the original delete

## Design note
- The client holds the same original operation ULID throughout the undo/redo cycle
- `undo(X)` applies the inverse of X
- `redo(X)` applies the forward of X
- The ChangeEntry X is immutable — undo and redo just pick which direction to apply
- Redo logs a new ChangeEntry with `op: "redo"` and `redone_id` referencing X

## Files
- `swissarmyhammer-entity/src/context.rs` — add `redo()` method

## Checklist
- [ ] Implement `EntityContext::redo()` — handle update, create, delete cases
- [ ] Redo of update: read entity, apply forward changes, write back
- [ ] Redo of create: restore from trash
- [ ] Redo of delete: delete entity
- [ ] Return error on stale redo (TextDiff patch doesn't apply)
- [ ] Write tests for each case
- [ ] Run full test suite