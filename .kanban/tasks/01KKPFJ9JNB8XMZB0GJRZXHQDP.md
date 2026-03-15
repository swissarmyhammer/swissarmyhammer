---
position_column: done
position_ordinal: '8580'
title: MCP server should auto-inject actor into kanban tool calls
---
## What
The MCP server creates an agent actor on `initialize` via `ensure_agent_actor()` but doesn't store the actor_id for injection into subsequent tool calls. When `add task` is called without an explicit `actor` arg, no auto-assignment happens.

The test `test_add_task_auto_assigns_actor` shows the expected behavior: when `actor` is in the args, the task gets auto-assigned. But the MCP server should inject this automatically from the session context.

**Files:**
- `swissarmyhammer-tools/src/mcp/server.rs` — store actor_id after `ensure_agent_actor`, inject into kanban tool calls
- `swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs` — verify actor arg is used for auto-assignment

**Approach:**
- After `ensure_agent_actor`, store the actor_id on the server/context
- Before dispatching kanban tool calls, inject `actor: <session_actor_id>` into args if not already present
- This makes all task creation auto-assign to the MCP session's actor

## Acceptance Criteria
- [ ] Tasks created via MCP are auto-assigned to the session's actor
- [ ] Explicit assignees still override auto-assignment
- [ ] All tests pass