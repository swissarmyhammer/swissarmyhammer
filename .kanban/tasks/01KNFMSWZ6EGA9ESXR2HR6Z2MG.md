---
assignees:
- claude-code
position_column: todo
position_ordinal: 9c80
title: 'NIT: PerspectiveTabBar commitRename deletes then saves — not atomic'
---
**File**: kanban-app/ui/src/components/perspective-tab-bar.tsx (commitRename function)\n\n**What**: Rename is implemented as `delete(oldName)` then `save(newName)`. If the save fails, the old perspective is already deleted. There is no rollback.\n\n**Suggestion**: Consider a single `perspective.rename` command that handles this atomically in the backend.\n\n**Subtasks**:\n- [ ] Add perspective.rename command to the backend\n- [ ] Update commitRename to use the atomic rename command\n- [ ] Verify fix by running tests #review-finding