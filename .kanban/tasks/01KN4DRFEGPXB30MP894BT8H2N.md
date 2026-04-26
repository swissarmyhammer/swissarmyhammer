---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffc180
title: Add tests for execute_operation (dispatch.rs)
---
swissarmyhammer-kanban/src/dispatch.rs:42-475\n\nCoverage: 35.9% (60/167 lines)\n\nUncovered: 107 lines in execute_operation -- the main dispatch function that routes Operation structs to kanban context methods. This is the central command dispatcher and the single largest uncovered block.\n\nWhat to test: Unit tests exercising each Operation variant through execute_operation -- add/update/delete/move/archive for tasks, tags, columns, swimlanes, actors, attachments, and board operations. Mock or use a temp KanbanContext. #coverage-gap