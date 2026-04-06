---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffc680
title: Add tests for KanbanContext open/find and activity logging
---
swissarmyhammer-kanban/src/context.rs\n\nCoverage: 72.7% (189/260 lines)\n\nUncovered functions:\n- open / find (31 lines: 96-148) -- board discovery and opening\n- append_task_log / append_tag_log / append_actor_log / append_column_log / append_swimlane_log / append_board_log / append_log (activity logging, lines 362-417)\n- read_activity (lines 427-440)\n- seed_builtin_views (lines 519-526)\n- write_entity_generic / delete_entity_generic / read_entity_changelog (lines 575-610)\n- lock (lines 623-633)\n\nWhat to test: Open a KanbanContext in a temp dir, verify board metadata loads. Test append_log writes entries and read_activity reads them back. Test lock acquires and releases. Test write/delete generic entity operations. #coverage-gap