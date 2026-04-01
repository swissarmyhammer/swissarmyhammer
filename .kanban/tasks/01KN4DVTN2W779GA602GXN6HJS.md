---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffbd80
title: Add tests for UndoStack push/load/save/clear
---
swissarmyhammer-entity/src/undo_stack.rs\n\nCoverage: 57.6% (34/59 lines)\n\nUncovered functions:\n- with_max_size (2 lines: 69-71)\n- undo_target / redo_target (navigation, lines 92-101)\n- push (9 lines: 123-135) -- adding entries and trimming to max size\n- clear (3 lines: 160-162)\n- load / save (7 lines: 174-198) -- file persistence\n\nWhat to test: Create UndoStack, push entries, verify undo_target/redo_target return correct entries. Test push trims when exceeding max_size. Test save to temp file and load back. Test clear empties the stack. #coverage-gap