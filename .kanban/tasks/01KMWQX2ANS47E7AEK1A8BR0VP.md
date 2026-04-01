---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffe380
title: Add undo/redo test for column.reorder command
---
swissarmyhammer-kanban/tests/command_dispatch_integration.rs\n\n`column.reorder` is `undoable: true` but has no undo/redo test.\n\nScenario:\n1. Board init creates columns (todo, doing, done) with specific ordinals\n2. Reorder columns via column.reorder (swaps doing and done, or moves done before doing)\n3. Verify new column order\n4. app.undo — verify original column order restored\n5. app.redo — verify reordered state restored\n\nNote: need to check how column.reorder takes its args — look at ColumnReorderCmd in column_commands.rs.\n\nFollow the TestEngine pattern in command_dispatch_integration.rs. #coverage-gap