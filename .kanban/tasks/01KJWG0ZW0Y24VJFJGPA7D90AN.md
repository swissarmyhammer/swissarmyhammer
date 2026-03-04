---
position_column: done
position_ordinal: i2
title: Comprehensive undo/redo test suite for entity layer
---
The user specifically emphasized thorough testing of many field-level set/undo/set/redo combinations and sequences. Tag rename is the heaviest transaction in the system — one rename touches the tag entity + N task bodies. This card builds a comprehensive test suite.

## Test categories

### Basic round-trips
- [ ] set field → undo → field restored to previous value
- [ ] set field → undo → redo → field has new value again
- [ ] create entity → undo → entity gone → redo → entity back
- [ ] delete entity → undo → entity restored → redo → entity gone again

### Multi-step sequences
- [ ] set A → set B → undo (B reverts) → undo (A reverts) → redo (A reapplied) → redo (B reapplied)
- [ ] set field → set same field again → undo → intermediate value restored (not original)
- [ ] set field1 → set field2 → undo field2 → field1 still has its new value

### Field type coverage
- [ ] String fields (TextDiff patches) — undo/redo with text changes
- [ ] Non-string fields (Changed with old/new JSON) — numbers, booleans, arrays
- [ ] Field addition (Set) → undo removes field → redo adds it back
- [ ] Field removal (Removed) → undo re-adds field → redo removes it again
- [ ] Multi-line text with scattered edits — TextDiff forward/reverse patches

### Tag rename transaction sequences (critical path)
This is the heaviest transaction in the system. One rename touches tag entity + N task bodies.

- [ ] Create 5 tasks with #frontend in body → rename tag to #fe → undo → all 5 tasks have #frontend again, tag name is #frontend
- [ ] Same as above → undo → redo → all 5 tasks have #fe, tag name is #fe
- [ ] Rename #frontend → #fe → rename #fe → #f → undo (back to #fe across all tasks) → undo (back to #frontend across all tasks)
- [ ] Rename #frontend → #fe → add new task with #fe in body → undo rename → original 5 tasks get #frontend back, new task's #fe is unchanged (it wasn't part of the original transaction)
- [ ] Two tags: #frontend and #backend, 3 tasks reference both. Rename #frontend → #fe, then rename #backend → #be. Undo #be rename → #backend restored in all 3 tasks, #fe unchanged. Undo #fe rename → #frontend restored in all 3 tasks, #backend already restored.
- [ ] Rename tag → undo → modify a task body manually → redo rename — stale undo should hard error for the modified task (TextDiff patch won't apply)
- [ ] Rename tag that appears in task title (not just body) — verify title field also gets bulk-renamed and undo restores it
- [ ] Rename tag to a name that partially overlaps with another tag (e.g. #front → #frontend when #frontend already exists) — verify no cross-contamination on undo

### Multiple tag operations
- [ ] Create tag → rename tag → delete tag → undo delete (tag restored with renamed name) → undo rename (tag back to original name) → undo create (tag gone)
- [ ] Delete tag that appears in 3 tasks → undo → tag restored AND all 3 task bodies have #tag re-inserted

### Edge cases
- [ ] Undo after entity was modified by another operation (stale undo — hard error)
- [ ] Undo the same operation twice — should error or no-op
- [ ] Redo without prior undo — should error
- [ ] Undo of create when entity has been modified since creation — what happens?
- [ ] Undo of delete when trash files have been manually removed — graceful error
- [ ] Empty changes (write with no actual diff) — no changelog entry, nothing to undo

### Delete/restore cycles
- [ ] delete → undo → entity readable again with all original fields
- [ ] delete → undo → modify entity → redo delete — should this work?
- [ ] delete → undo → delete again (new operation) → undo new delete

## Files
- `swissarmyhammer-entity/src/context.rs` — tests module
- `swissarmyhammer-entity/tests/undo_redo.rs` — integration test file
- `swissarmyhammer-kanban/tests/tag_rename_undo.rs` — kanban-level transaction tests

## Checklist
- [ ] Write all entity-level test cases
- [ ] Write all tag rename transaction test cases
- [ ] Write all multi-tag sequence test cases
- [ ] Verify tests pass
- [ ] Run full test suite to ensure no regressions