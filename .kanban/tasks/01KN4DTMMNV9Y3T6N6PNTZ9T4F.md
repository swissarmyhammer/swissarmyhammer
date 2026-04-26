---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffd580
title: Add tests for entity_commands (archive, unarchive, tag update, delete)
---
swissarmyhammer-kanban/src/commands/entity_commands.rs\n\nCoverage: 27.3% (9/33 lines)\n\nUncovered functions:\n- DeleteEntityCmd::execute (3 lines: 51-59)\n- ArchiveEntityCmd::execute (5 lines: 87-111)\n- UnarchiveEntityCmd::execute (5 lines: 129-153)\n- TagUpdateCmd::available + execute (7 lines: 167-186)\n- AttachmentDeleteCmd::available + execute (3 lines: 202-207)\n\nWhat to test: Set up a KanbanContext with tasks/tags, then execute each command through the Command trait. Verify entities are deleted, archived, unarchived. Test TagUpdateCmd modifies tag fields. Test AttachmentDeleteCmd removes attachment. #coverage-gap