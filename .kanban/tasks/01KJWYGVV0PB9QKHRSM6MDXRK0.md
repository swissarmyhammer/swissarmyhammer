---
position_column: todo
position_ordinal: d0
title: Cache KanbanContext per MCP session instead of recreating per tool call
---
The MCP kanban tool creates a new KanbanContext on every tool call via `get_kanban_context()` in `swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:161-170`. This means the FieldsContext (21 fields, 7 entities) is rebuilt from YAML on every call — visible as `fields context built from YAML sources` in the logs on every operation.

The context should be created once per MCP session (or per working directory) and cached. The ToolContext or the tool struct itself should hold the cached context.

- [ ] Add cached KanbanContext to KanbanTool or ToolContext
- [ ] Initialize on first call, reuse on subsequent calls
- [ ] Verify fields context log only appears once per session
- [ ] Run full test suite