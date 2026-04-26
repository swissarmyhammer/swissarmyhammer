---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffbf80
title: Add test for UndoStack::clear()
---
swissarmyhammer-entity/src/undo_stack.rs:160\n\n`pub fn clear(&mut self)` — sets entries to empty vec and pointer to 0. No test calls this method.\n\nTest: push several entries, call clear(), verify can_undo() is false, can_redo() is false, entries are empty. #coverage-gap