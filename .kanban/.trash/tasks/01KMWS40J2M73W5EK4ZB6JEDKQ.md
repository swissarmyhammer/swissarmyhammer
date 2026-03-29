---
assignees:
- claude-code
depends_on:
- 01KMWS35SK4YDZ17FQG83PV1SH
- 01KMWS3KZAZHYCTF3KR86GG6BJ
position_column: todo
position_ordinal: 8a80
title: Clipboard command integration tests (execution + undo/redo)
---
## What

Add integration tests for clipboard commands through the TestEngine harness. Per `swissarmyhammer-commands/README.md`, undoable commands require undo/redo integration tests.

### Files to modify
- `swissarmyhammer-kanban/tests/command_dispatch_integration.rs` — add test functions

### Tests to write

**entity.copy execution test:**
1. Create a task with known fields
2. Dispatch `entity.copy` with task in scope
3. Verify UIState clipboard contains the task's fields with mode=Copy
4. Verify task still exists (not deleted)

**entity.cut execution + undo/redo test:**
1. Create a task with known fields
2. Dispatch `entity.cut` with task in scope
3. Verify UIState clipboard contains the task's fields with mode=Cut
4. Verify task is deleted
5. Dispatch `app.undo` — verify task is restored
6. Dispatch `app.redo` — verify task is deleted again

**entity.paste execution + undo/redo test:**
1. Create a task, dispatch `entity.copy`
2. Dispatch `entity.paste` with column in scope
3. Verify a new task exists in the target column with copied fields
4. Verify clipboard still has data (multi-paste)
5. Dispatch `app.undo` — verify pasted task is removed
6. Dispatch `app.redo` — verify pasted task reappears

**entity.cut → paste end-to-end test:**
1. Create a task in column A
2. Dispatch `entity.cut` (task deleted from column A)
3. Dispatch `entity.paste` with column B in scope (new task in column B)
4. Dispatch `app.undo` (paste undone — task gone from B)
5. Dispatch `app.undo` (cut undone — original task restored in A)

## Acceptance Criteria
- [ ] Copy execution test passes
- [ ] Cut execution + undo/redo test passes
- [ ] Paste execution + undo/redo test passes
- [ ] Cut → paste end-to-end with sequential undo passes
- [ ] `cargo nextest run -E 'rdeps(swissarmyhammer-kanban)'` all green

## Tests
- [ ] `swissarmyhammer-kanban/tests/command_dispatch_integration.rs` — 4 test functions as described above
- [ ] `cargo nextest run --package swissarmyhammer-kanban --test command_dispatch_integration` passes"
<parameter name="assignees">[]