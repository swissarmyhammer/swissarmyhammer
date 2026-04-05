---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff9780
title: Test kanban task complete/list/paste/update/unassign — below 80%
---
Files:\n- swissarmyhammer-kanban/src/task/complete.rs: 14/28 (50%) — affected_resource_ids untested\n- swissarmyhammer-kanban/src/task/list.rs: 26/39 (66.7%) — with_swimlane, with_tag, with_assignee filters untested\n- swissarmyhammer-kanban/src/task/paste.rs: 50/65 (76.9%) — partial coverage on paste logic\n- swissarmyhammer-kanban/src/task/update.rs: 32/47 (68.1%) — with_swimlane, with_assignees untested\n- swissarmyhammer-kanban/src/task/unassign.rs: 14/19 (73.7%) — affected_resource_ids untested\n\nNeed tests for task list filtering, complete/unassign resource tracking, paste edge cases, and update with swimlane/assignees.\n\n#coverage-gap #coverage-gap