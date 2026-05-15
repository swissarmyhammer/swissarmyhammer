---
position_column: done
position_ordinal: ffffff8280
title: Auto-assign tasks to agent actor on creation
---
When an agent creates a task via the kanban system, it should be automatically assigned to itself.

- [ ] Find the `AddTask` operation in `swissarmyhammer-kanban/src/task/`
- [ ] Check for an active actor context or session actor ID
- [ ] If the creating actor is an agent, add it to assignees automatically
- [ ] Add test for auto-assignment behavior