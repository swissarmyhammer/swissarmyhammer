---
assignees:
- claude-code
depends_on:
- 01KS865E8GE912VE7KSW836N8V
position_column: todo
position_ordinal: 8b80
project: ai-panel
title: 'llama-agent: bridge MCP elicitation to the ACP client (elicitation/create)'
---
#elicitation #llama-agent

## Context / Why
The local-llama backend (`crates/llama-agent`) is an in-process Rust ACP **Agent** whose own MCP client owns the connection to the per-board SAH MCP server (`mcpUrl` → `MCPServerConfig::Http`). For elicitation parity with the Claude backend, llama-agent must (a) advertise the elicitation capability to the MCP server so the server's `peer.create_elicitation(...)` is allowed, and (b) when the SAH server sends an `elicitation/create` during a tool call, redirect it to the user by emitting an ACP `elicitation/create` request to the webview, then return the user's response to the MCP server.

This is the in-process Rust analog of the claude-agent task; here we control both the MCP client and the ACP agent side, so the whole round-trip lives in this crate. Use the existing ACP send-request mechanism (the agent holds a `ConnectionTo<Client>` / `cx`) to deliver the elicitation, mirroring how the permission/other agent→client requests are sent in claude-agent's `request_user_permission`.

## Dependency
Requires the ACP elicitation types — depends on "Enable ACP elicitation types in the workspace". Use `agent_client_protocol::schema::{CreateElicitationRequest, CreateElicitationResponse, ElicitationAction}` (do NOT add feature flags here; the enablement task handles that). The MCP side uses rmcp's elicitation types (mirror `crates/swissarmyhammer-tools/src/mcp/tools/questions/ask/mod.rs`, the sender). ACP method name is `elicitation/create`.

## What
- [ ] **Investigate first (subtask):** Locate llama-agent's MCP client setup and where MCP server→client requests are handled; identify where the ACP `ConnectionTo<Client>`/`cx` is available to send agent→client requests during a tool call. Document in task comments.
- [ ] Make the MCP client advertise elicitation capability in its MCP `initialize`.
- [ ] On an inbound MCP `elicitation/create`, translate to an ACP `CreateElicitationRequest` (form mode; with `tool_call_id` when known) and send to the webview; await the `CreateElicitationResponse`.
- [ ] Translate the ACP response (`ElicitationAction` accept{content}/decline/cancel) back into the rmcp elicitation result returned to the SAH MCP server.
- [ ] Graceful fallback (decline) if no ACP client is connected.

## Acceptance Criteria
- [ ] llama-agent's MCP client advertises elicitation capability.
- [ ] An MCP `elicitation/create` produces one ACP `elicitation/create` to the client and relays the response back to the MCP server.
- [ ] Decline/cancel propagate as the correct rmcp elicitation actions.

## Tests (`crates/llama-agent/...`)
- [ ] Integration test with a fake ACP client + a stub MCP elicitation: assert the ACP `elicitation/create` request is emitted and the response round-trips to the MCP layer (accept/decline/cancel).
- [ ] Run: `cargo nextest run -p llama-agent` — all green.

## Workflow
- Use `/tdd`. Investigate the MCP-client + cx wiring, write the failing fake-client test, then implement.