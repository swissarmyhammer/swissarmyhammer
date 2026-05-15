---
assignees:
- claude-code
depends_on:
- 01KM14V2VKA9J6JBA15QF1JASG
position_column: done
position_ordinal: ffffffffffb280
title: Wire tool config loading into MCP server startup
---
## What

Load `tools.yaml` during `McpServer` initialization and apply the disabled set to the `ToolRegistry` before the server starts accepting connections.

**Integration point:** `McpServer::initialize_tools()` in `swissarmyhammer-tools/src/mcp/server.rs` (around line 728). After `register_all_tools()` completes, load the tool config and call `set_tool_enabled(name, false)` for each disabled tool.

**Loading sequence:**
1. `register_all_tools()` — all tools registered (existing behavior)
2. `remove_agent_tools()` — if not agent mode (existing behavior)
3. **NEW:** `load_and_apply_tool_config()` — read tools.yaml, disable configured tools

**Config resolution:** Use `swissarmyhammer-directory` to find `.sah/` at project root and `~/.sah/` for global. Load both, merge (project overrides global), apply.

**Files:**
- `swissarmyhammer-tools/src/mcp/server.rs` — call config loader after tool registration

## Acceptance Criteria
- [ ] Server startup loads tools.yaml and applies disabled set
- [ ] Missing tools.yaml is not an error (all tools enabled by default)
- [ ] Malformed tools.yaml logs a warning but doesn't crash the server
- [ ] Project-level tools.yaml overrides global

## Tests
- [ ] Integration test: create temp tools.yaml disabling shell → start server → list_tools excludes shell
- [ ] Integration test: no tools.yaml → all tools visible
- [ ] `cargo nextest run -p swissarmyhammer-tools` #tool-filter