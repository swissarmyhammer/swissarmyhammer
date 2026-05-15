---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffc280
title: Add undo/redo test for task.move command
---
swissarmyhammer-kanban/tests/command_dispatch_integration.rs\n\n`task.move` is `undoable: true` but has no undo/redo test.\n\nScenario:\n1. Add a task to column \"todo\"\n2. Move it to column \"doing\" via task.move with scope [\"task:{id}\", \"column:todo\"]\n3. Verify task is in \"doing\"\n4. app.undo — verify task is back in \"todo\" with original ordinal\n5. app.redo — verify task is in \"doing\" again\n\nFollow the TestEngine pattern in command_dispatch_integration.rs. #coverage-gap