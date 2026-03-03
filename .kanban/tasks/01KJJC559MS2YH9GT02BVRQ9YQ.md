---
title: Remove subtask MCP tool registration and dispatch
position:
  column: done
  ordinal: b1
---
Remove all subtask operation wiring from the MCP tools layer.

**Files to modify:**
- `swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs`:
  - Remove 4 static lazy instances (ADD_SUBTASK, COMPLETE_SUBTASK, DELETE_SUBTASK, UPDATE_SUBTASK)
  - Remove from KANBAN_OPERATIONS array
  - Remove from task-modifying operations match arm
  - Remove dispatch handlers (~lines 824-871)
  - Remove subtask-related use/import statements

## Checklist
- [ ] Remove static lazy subtask instances
- [ ] Remove from KANBAN_OPERATIONS array
- [ ] Remove from task-modifying operations match
- [ ] Remove subtask dispatch handlers
- [ ] Remove imports
- [ ] Run tests