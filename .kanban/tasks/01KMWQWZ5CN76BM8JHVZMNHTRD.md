---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffc880
title: Add undo/redo test for entity.archive and entity.unarchive commands
---
swissarmyhammer-kanban/tests/command_dispatch_integration.rs\n\nBoth `entity.archive` and `entity.unarchive` are `undoable: true` but have no undo/redo tests.\n\nScenario for archive:\n1. Add a task\n2. Archive it via entity.archive with target \"task:{id}\"\n3. Verify task is not in normal list (read returns error or list doesn't include it)\n4. app.undo — verify task is restored from archive\n5. app.redo — verify task is archived again\n\nScenario for unarchive:\n1. Add a task, archive it\n2. Unarchive it via entity.unarchive with target \"task:{id}\"\n3. Verify task is back in normal list\n4. app.undo — verify task is archived again\n5. app.redo — verify task is unarchived again\n\nFollow the TestEngine pattern in command_dispatch_integration.rs. #coverage-gap