---
assignees:
- claude-code
position_column: todo
position_ordinal: '8380'
title: Add undo/redo test for entity.delete command
---
swissarmyhammer-kanban/tests/command_dispatch_integration.rs\n\n`entity.delete` is `undoable: true` but has no undo/redo test. This is the generic entity delete (vs task-specific task.delete).\n\nScenario:\n1. Verify a tag entity exists (e.g. one of the default tags created by board init)\n2. Delete it via entity.delete with target \"tag:{id}\"\n3. Verify tag is gone\n4. app.undo — verify tag is restored with original fields\n5. app.redo — verify tag is gone again\n\nFollow the TestEngine pattern in command_dispatch_integration.rs. #coverage-gap