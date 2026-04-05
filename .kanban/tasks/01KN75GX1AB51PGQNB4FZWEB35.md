---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff9e80
title: Test kanban column get/list/update/delete — low coverage
---
Files:\n- swissarmyhammer-kanban/src/column/get.rs: 0/7 (0%)\n- swissarmyhammer-kanban/src/column/list.rs: 0/5 (0%)\n- swissarmyhammer-kanban/src/column/update.rs: 12/17 (70.6%)\n- swissarmyhammer-kanban/src/column/delete.rs: 10/15 (66.7%)\n\nColumn Execute impls for get, list are completely untested. Update and delete have partial coverage. Need tests for column retrieval, listing, name/order update, and delete-with-tasks error.\n\n#coverage-gap #coverage-gap