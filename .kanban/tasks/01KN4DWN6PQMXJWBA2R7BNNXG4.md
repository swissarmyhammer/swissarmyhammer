---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffc780
title: Add tests for MoveTaskCmd ordinal positioning
---
swissarmyhammer-kanban/src/commands/task_commands.rs\n\nCoverage: 53.7% (36/67 lines)\n\nUncovered functions:\n- MoveTaskCmd::execute (25 lines: 72-246) -- the largest gap; handles column moves, before_id/after_id ordinal computation, swimlane moves\n- AddTaskCmd::execute (2 lines: 28-29)\n- UntagTaskCmd::execute (2 lines: 278-281)\n- DeleteTaskCmd::execute (2 lines: 305-306)\n\nWhat to test: Create board with multiple tasks, move a task to a different column and verify ordinal. Test before_id/after_id positioning. Test move to swimlane. Test untag and delete commands. #coverage-gap