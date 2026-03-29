---
assignees:
- claude-code
position_column: todo
position_ordinal: '8180'
title: Add undo/redo test for task.delete command
---
swissarmyhammer-kanban/tests/command_dispatch_integration.rs\n\n`task.delete` is `undoable: true` but has no undo/redo test.\n\nScenario:\n1. Add a task to column \"todo\"\n2. Delete it via task.delete with scope [\"task:{id}\"]\n3. Verify task is gone (read returns error)\n4. app.undo — verify task is restored with its original fields\n5. app.redo — verify task is gone again\n\nFollow the TestEngine pattern in command_dispatch_integration.rs. #coverage-gap