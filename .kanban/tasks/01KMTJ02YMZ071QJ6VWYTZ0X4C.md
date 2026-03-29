---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffba80
title: Wire UndoStack into EntityContext write/delete paths
---
## What

After every successful `write()` or `delete()` call in EntityContext, push the resulting changelog entry ID (or transaction ID if active) onto the UndoStack and flush to disk. After `undo()` and `redo()`, update the pointer and flush.

**Key changes in `swissarmyhammer-entity/src/context.rs`:**
- In `write()`: after appending the changelog entry, call `self.undo_stack.push(entry)` then `self.undo_stack.save(&self.undo_stack_path())` — use the transaction_id if one is active, otherwise the entry ULID. Label comes from the op + entity type + entity id.
- In `delete()`: same pattern
- In `undo()`: after successful undo, call `self.undo_stack.record_undo()` then `save()`
- In `redo()`: after successful redo, call `self.undo_stack.record_redo()` then `save()`
- Deduplicate transaction pushes: if the same transaction_id is already the top of the stack, don't push again (multiple writes in one transaction)
- Add `undo_stack_path()` helper: returns `{root}/undo_stack.yaml`

Every mutation writes the YAML file so you can `cat .kanban/undo_stack.yaml` at any time to see the current state.

**Files to modify:**
- `swissarmyhammer-entity/src/context.rs` — wire push/undo/redo into existing methods, add save calls
- `swissarmyhammer-entity/src/undo_stack.rs` — add `record_undo()` and `record_redo()` methods

## Acceptance Criteria
- [ ] After a write, the entry/transaction ID is on the undo stack AND on disk
- [ ] After undo, the pointer moves back AND the YAML file reflects it
- [ ] After redo, the pointer moves forward AND the YAML file reflects it
- [ ] Transaction dedup: multiple writes in same transaction produce one stack entry
- [ ] `cat .kanban/undo_stack.yaml` shows current stack state at any time
- [ ] Existing undo/redo tests still pass

## Tests
- [ ] Integration test: write → read YAML file → verify entry present with correct pointer
- [ ] Integration test: undo → read YAML → verify pointer decremented
- [ ] `cargo nextest run -p swissarmyhammer-entity` passes