---
depends_on: []
position_column: todo
position_ordinal: a1
title: Wire query parameter in MCP kanban dispatch + schema
---
## What
Wire the `query` parameter through the MCP tool dispatch layer so that `op: "search tasks", query: "bug"` works.

Files:
- `swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs` — in the `(Verb::List, Noun::Tasks)` match arm (~line 610), extract `query` string param and call `cmd = cmd.with_query(query)`
- `swissarmyhammer-kanban/src/schema.rs` — add `query` as an optional string parameter to the `list tasks` operation schema

## Acceptance Criteria
- [ ] `op: "search tasks", query: "foo"` returns only matching tasks via MCP tool
- [ ] `op: "list tasks", query: "foo"` also works (search is alias for list)
- [ ] `query` parameter appears in the MCP tool schema
- [ ] Query composes with existing params (column, tag, etc.)

## Tests
- [ ] MCP integration test: create tasks, search by query, verify filtered results
- [ ] Existing MCP kanban tests still pass