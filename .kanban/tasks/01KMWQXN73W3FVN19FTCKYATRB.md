---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffe580
title: Add undo/redo test for attachment.delete command
---
swissarmyhammer-kanban/tests/command_dispatch_integration.rs\n\n`attachment.delete` is `undoable: true` but has no undo/redo test.\n\nScenario:\n1. Add a task\n2. Add an attachment to the task (may need to create a temp file and use the attachment add flow)\n3. Delete the attachment via attachment.delete with args { task_id, id }\n4. Verify attachment is gone from the task\n5. app.undo — verify attachment is restored\n6. app.redo — verify attachment is gone again\n\nNote: check how AttachmentDeleteCmd works in entity_commands.rs to understand the args and what gets deleted (entity reference vs actual file).\n\nFollow the TestEngine pattern in command_dispatch_integration.rs. #coverage-gap