---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffb580
title: Add tests for MoveTaskCmd swimlane arg
---
task_commands.rs:254\n\n`if let Some(swimlane) = ctx.arg(\"swimlane\")...`\n\nThe swimlane argument handling in MoveTaskCmd::execute is never exercised. No test passes a swimlane arg when dispatching task.move.\n\nTest should:\n- Create a board with swimlanes\n- Move a task with a swimlane arg\n- Verify the task ends up in the target swimlane #coverage-gap