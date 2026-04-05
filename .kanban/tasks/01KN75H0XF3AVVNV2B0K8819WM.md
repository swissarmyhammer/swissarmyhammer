---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffff980
title: Test kanban drag and entity commands — 22-28% coverage
---
Files:\n- swissarmyhammer-kanban/src/commands/drag_commands.rs: 14/62 (22.6%)\n- swissarmyhammer-kanban/src/commands/entity_commands.rs: 13/47 (27.7%)\n\nDrag commands: DragCancelCmd::available and most drag logic untested.\nEntity commands: DeleteEntityCmd::execute, ArchiveEntityCmd::execute, UnarchiveEntityCmd::execute, TagUpdateCmd, AttachmentOpenCmd::execute, AttachmentRevealCmd::execute all uncovered.\n\nThese are UI command handlers with real logic. Need tests.\n\n#coverage-gap #coverage-gap