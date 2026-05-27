---
assignees:
- claude-code
position_column: todo
position_ordinal: '8180'
project: ai-panel
title: Verify ACP agents honor NewSessionRequest.mcp_servers
---
## What
The AI panel hands the kanban MCP server to the agent strictly over ACP — in the `NewSessionRequest.mcp_servers` field — not through a side channel. But `swissarmyhammer-agent::create_claude_agent` currently bakes MCP config into the claude-agent config at agent-creation time (for its one-shot `execute_prompt` path). This task confirms the ACP backends honor the per-session field, and closes the gap if they do not.

- Inspect `crates/claude-agent/src/agent_trait_impl.rs` and `session.rs` `new_session` handling; inspect `crates/llama-agent/src/acp/` session creation.
- Determine whether each backend reads `NewSessionRequest.mcp_servers` and connects those MCP servers for that session.
- If a backend ignores the ACP field, fix it IN THE AGENT CRATE so a session created with `mcp_servers` connects them. The kanban-app must never work around a gap with a side channel — that violates the "strictly ACP" rule.

Spec: `ideas/kanban/ai_panel.md` — Phase 2 "MCP servers travel over ACP", Open Question 5.

## Acceptance Criteria
- [ ] Task comments record whether `claude-agent` and `llama-agent` honor `NewSessionRequest.mcp_servers`.
- [ ] If a gap exists, it is fixed in the agent crate: `new_session` with a populated `mcp_servers` connects those servers for the session.
- [ ] An ACP session created with one MCP server in `mcp_servers` exposes that server's tools to the agent.

## Tests
- [ ] Integration test in `crates/claude-agent/tests/` (or `crates/acp-conformance`): start a test MCP server (see `agent-client-protocol-extras::start_test_mcp_server`), create an ACP session passing it in `NewSessionRequest.mcp_servers`, assert the agent can list/call that server's tool.
- [ ] Equivalent coverage for `llama-agent` if it is in scope for this gap.
- [ ] `cargo test -p claude-agent` is green.

## Workflow
- Use `/tdd` — write the failing integration test that exercises `mcp_servers` at `new_session` first, then implement the fix.