---
position_column: done
position_ordinal: ffffdc80
title: Auto-create MCP agent actor on initialize
---
When an MCP client connects, auto-create an agent actor from the client info.

## Changes
- `swissarmyhammer-tools/src/mcp/server.rs` — in `initialize()` handler (~line 920), after file watching setup, create actor

## Design
- Actor ID = slugified `request.client_info.name` (e.g. "claude-code")
- Display name = `request.client_info.name` (or `title` if present)
- `actor_type: agent`, `ensure: true`
- Derive deterministic color from name hash
- Generate geometric/robot SVG avatar to visually distinguish from human actors
- McpServer has access to KanbanContext through its board handle

## Subtasks
- [ ] Extract client_info.name in initialize() handler
- [ ] Slugify name for actor ID
- [ ] Execute AddActor::agent with ensure, color, avatar
- [ ] Verify reconnection is idempotent
- [ ] Run `cargo test`