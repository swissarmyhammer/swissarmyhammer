---
position_column: done
position_ordinal: v2
title: Remove default 'assistant' actor — use MCP agent session identity only
---
The kanban board auto-creates a default 'assistant' actor. Remove this — actors for AI agents should only be created via MCP agent session identification (Card 3: auto-create MCP agent actor on initialize). The OS user actor (Card 2) is fine.\n\n## Subtasks\n- [ ] Find and remove default 'assistant' actor creation\n- [ ] Verify MCP agent actor creation still works\n- [ ] Run tests