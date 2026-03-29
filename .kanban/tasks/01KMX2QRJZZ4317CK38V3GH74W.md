---
assignees:
- claude-code
depends_on:
- 01KMX2QA5T6G1A4HG1BE662M4D
position_column: todo
position_ordinal: '8580'
title: Clipboard undo/redo integration tests
---
## What

Integration tests for clipboard commands through TestEngine, verifying undo/redo semantics and system clipboard integration.

### Files to modify
- `swissarmyhammer-kanban/tests/command_dispatch_integration.rs` — add test functions

### Tests to write

**entity.copy execution test:**
1. Create a task with known fields
2. Dispatch entity.copy with task in scope
3. Verify system clipboard (InMemoryClipboard) contains the task's fields as JSON
4. Verify task still exists (not deleted)

**entity.cut execution + undo/redo test:**
1. Create a task with known fields
2. Dispatch entity.cut with task in scope
3. Verify system clipboard has the task snapshot
4. Verify task is deleted
5. app.undo → verify task restored, clipboard still populated
6. app.redo → verify task deleted again

**entity.paste execution + undo/redo test:**
1. Create a task, dispatch entity.copy
2. Dispatch entity.paste with column in scope
3. Verify new task exists with new ID and copied fields
4. Verify clipboard still has data (multi-paste)
5. app.undo → pasted task removed
6. app.redo → pasted task reappears

**Cut → paste end-to-end test:**
1. Create task in column A
2. entity.cut (deleted from A, on clipboard)
3. entity.paste with column B (new task in B)
4. app.undo (paste undone — gone from B)
5. app.undo (cut undone — original restored in A)

## Acceptance Criteria
- [ ] All 4 test scenarios pass
- [ ] Tests use InMemoryClipboard (no real system clipboard)
- [ ] Cut and paste are independent undo steps
- [ ] Copy never appears in undo stack
- [ ] `cargo nextest run -p swissarmyhammer-kanban --test command_dispatch_integration` passes

## Tests
- [ ] 4 integration test functions as described above"
<parameter name="assignees">[]