---
assignees:
- claude-code
position_column: todo
position_ordinal: 8b80
project: ai-panel
title: 'llama-agent: bridge MCP elicitation to the ACP client (session/elicitation)'
---
#elicitation #llama-agent

## Context / Why
The local-llama backend (`crates/llama-agent`) is an in-process Rust ACP **Agent** whose own MCP client owns the connection to the per-board SAH MCP server (`mcpUrl` → `MCPServerConfig::Http`). For elicitation parity with the Claude backend, llama-agent must (a) advertise the elicitation capability to the MCP server so the server's `peer.create_elicitation(...)` is allowed, and (b) when the SAH server sends an `elicitation/create` during a tool call, redirect it to the user by emitting an ACP `session/elicitation` request to the webview, then return the user's response to the MCP server.

This is the in-process Rust analog of the claude-agent task; here we control both the MCP client and the ACP agent side, so the whole round-trip lives in this crate. Use the existing ACP send-request mechanism (the agent holds a `ConnectionTo<Client>` / `cx`) to deliver the elicitation, mirroring how permission/other agent→client requests are sent.

ACP Rust elicitation types are behind the `unstable_elicitation` feature of `agent-client-protocol` 0.11 (`schema::ElicitationRequest`, `ElicitationResponse`, `ElicitationAction`, `ElicitationMode::Form{requested_schema}`). The MCP side uses rmcp's elicitation types (mirror `crates/swissarmyhammer-tools/src/mcp/tools/questions/ask/mod.rs`, which is the sender).

## What
- [ ] **Investigate first (subtask):** Locate llama-agent's MCP client setup and where MCP server→client requests are handled; identify where the ACP `ConnectionTo<Client>`/`cx` is available to send agent→client requests during a tool call. Document in task comments.
- [ ] Enable `unstable_elicitation` for `crates/llama-agent`.
- [ ] Make the MCP client advertise elicitation capability in its MCP `initialize`.
- [ ] On an inbound MCP `elicitation/create`, translate to a session-scoped ACP `ElicitationRequest` (with `tool_call_id` when known) and send to the webview; await the `ElicitationResponse`.
- [ ] Translate the ACP response (Accept{content}/Decline/Cancel) back into the rmcp elicitation result returned to the SAH MCP server.
- [ ] Graceful fallback (decline) if no ACP client is connected.

## Acceptance Criteria
- [ ] llama-agent's MCP client advertises elicitation capability.
- [ ] An MCP `elicitation/create` produces one ACP `session/elicitation` to the client and relays the response back to the MCP server.
- [ ] Decline/cancel propagate as the correct rmcp elicitation actions.

## Tests (`crates/llama-agent/...`)
- [ ] Integration test with a fake ACP client + a stub MCP elicitation: assert the ACP `session/elicitation` request is emitted and the response round-trips to the MCP layer (accept/decline/cancel).
- [ ] Run: `cargo nextest run -p llama-agent` — all green.

## Workflow
- Use `/tdd`. Investigate the MCP-client + cx wiring, write the failing fake-client test, then implement.