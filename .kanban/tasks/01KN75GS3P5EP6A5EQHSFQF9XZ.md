---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffde80
title: Test kanban swimlane CRUD operations — 0% coverage across 5 files
---
Files:\n- swissarmyhammer-kanban/src/swimlane/delete.rs: 0/15 (0%)\n- swissarmyhammer-kanban/src/swimlane/get.rs: 0/7 (0%)\n- swissarmyhammer-kanban/src/swimlane/list.rs: 0/5 (0%)\n- swissarmyhammer-kanban/src/swimlane/update.rs: 0/17 (0%)\n- swissarmyhammer-kanban/src/swimlane/add.rs: 16/22 (72.7%)\n\nAll swimlane Execute impls are untested. Need tests for delete (including fails-if-has-tasks), get, list, update (name/order), and add edge cases.\n\n#coverage-gap #coverage-gap