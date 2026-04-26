---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffc680
title: Add undo/redo test for task.untag command
---
swissarmyhammer-kanban/tests/command_dispatch_integration.rs\n\n`task.untag` is `undoable: true` but has no undo/redo test.\n\nScenario:\n1. Add a task, tag it with a tag (via task description containing #bug or via entity.update_field on the tags field)\n2. Untag it via task.untag with scope [\"tag:{tag_id}\", \"task:{task_id}\"]\n3. Verify tag is removed from task body\n4. app.undo — verify tag is restored in task body\n5. app.redo — verify tag is removed again\n\nFollow the TestEngine pattern in command_dispatch_integration.rs. #coverage-gap