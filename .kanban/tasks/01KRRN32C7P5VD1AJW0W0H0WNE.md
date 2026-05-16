---
assignees:
- claude-code
position_column: todo
position_ordinal: '8280'
project: ai-panel
title: Verify the ACP agent honors an HTTP MCP server in newSession.mcpServers
---
## What
The TypeScript ACP client gives the agent the full SwissArmyHammer toolset by putting an HTTP MCP server entry — `McpServer::Http` pointing at the board's in-process SAH toolset URL — in the ACP `newSession` request's `mcpServers` array. Confirm the Rust ACP agent honors HTTP `mcpServers` entries per session and connects them.

- `claude-agent` has `convert_acp_to_internal_mcp_config` (`crates/claude-agent/src/agent.rs`) and uses `claude_agent::config::McpServerConfig::Http` — verify it actually connects an HTTP `mcpServers` entry from `newSession` and exposes its tools.
- Verify the llama-agent ACP path (`crates/llama-agent/src/acp/`) reads and connects HTTP `mcpServers` likewise.
- If either backend ignores the field, fix it in the agent crate. The kanban-app must not work around a gap.

## Acceptance Criteria
- [ ] Task comments record whether `claude-agent` and `llama-agent` honor an HTTP entry in `newSession.mcpServers`.
- [ ] If a gap exists it is fixed in the agent crate so a session created with an HTTP `mcpServers` entry connects it.
- [ ] A session created with one HTTP MCP server in `mcpServers` exposes that server's tools to the agent.

## Tests
- [ ] Integration test (`crates/claude-agent/tests/` or `crates/acp-conformance`): start a test HTTP MCP server, create a session passing it as an `McpServer::Http` in `newSession.mcpServers`, assert the agent can list/call that server's tool.
- [ ] Equivalent coverage for the llama-agent path if a gap is found there.
- [ ] `cargo test` for the affected crate(s) is green.

## Workflow
- Use `/tdd` — write the failing HTTP-`mcpServers`-at-`newSession` integration test first.