---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffee80
project: ai-panel
title: Verify the ACP agent honors an HTTP MCP server in newSession.mcpServers
---
## What
The TypeScript ACP client gives the agent the full SwissArmyHammer toolset by putting an HTTP MCP server entry — `McpServer::Http` pointing at the board's in-process SAH toolset URL — in the ACP `newSession` request's `mcpServers` array. Confirm the Rust ACP agent honors HTTP `mcpServers` entries per session and connects them.

- `claude-agent` has `convert_acp_to_internal_mcp_config` (`crates/claude-agent/src/agent.rs`) and uses `claude_agent::config::McpServerConfig::Http` — verify it actually connects an HTTP `mcpServers` entry from `newSession` and exposes its tools.
- Verify the llama-agent ACP path (`crates/llama-agent/src/acp/`) reads and connects HTTP `mcpServers` likewise.
- If either backend ignores the field, fix it in the agent crate. The kanban-app must not work around a gap.

## Findings (verification)
- **llama-agent: HONORS it.** `crates/llama-agent/src/acp/server.rs::new_session` merges `config.default_mcp_servers` with `request.mcp_servers`, then for each calls `mcp_client_factory::create_mcp_client_from_acp`. The `McpServer::Http` arm builds a `UnifiedMCPClient::with_streamable_http_and_handler`, tools are discovered via `list_tools_with_schemas`, added to `session.available_tools`, and the clients stored in `session_mcp_clients`. No gap.
- **claude-agent: GAP — FIXED.** `new_session` only (1) validated transport capability via `validate_new_session_mcp_config` and (2) serialized the servers into the `Session` struct via `store_mcp_servers_in_session`. `McpServerManager::connect_servers` was never called in production code — only from unit tests. The `ToolCallHandler` already reads tools from the shared `mcp_manager` (`list_available_tools`) and routes calls to it (`execute_tool_call`), but the manager stayed empty, so an HTTP `mcpServers` entry from `newSession` was never connected and its tools never exposed.
- **Fix (claude-agent crate only):** Added `connect_new_session_mcp_servers` to `ClaudeAgent`, invoked from `new_session`. It converts each ACP `McpServer` via `convert_acp_to_internal_mcp_config` and connects them through the shared `mcp_manager`. `McpServerManager::connect_servers` changed from `&mut self` to `&self` (interior mutability via `Arc<RwLock<..>>` was already present) so it can be called on the shared `Arc<McpServerManager>`. Connected tools then flow to the agent through the existing `ToolCallHandler` -> `mcp_manager.list_available_tools()` path.

## Acceptance Criteria
- [x] Task comments record whether `claude-agent` and `llama-agent` honor an HTTP entry in `newSession.mcpServers`.
- [x] If a gap exists it is fixed in the agent crate so a session created with an HTTP `mcpServers` entry connects it.
- [x] A session created with one HTTP MCP server in `mcpServers` exposes that server's tools to the agent.

## Tests
- [x] Integration test (`crates/claude-agent/tests/`): start a test HTTP MCP server, create a session passing it as an `McpServer::Http` in `newSession.mcpServers`, assert the agent can list/call that server's tool.
- [x] Equivalent coverage for the llama-agent path if a gap is found there. (No gap found in llama-agent; existing llama-agent ACP coverage stands. claude-agent gap covered by new test.)
- [x] `cargo test` for the affected crate(s) is green.

## Workflow
- Use `/tdd` — write the failing HTTP-`mcpServers`-at-`newSession` integration test first.