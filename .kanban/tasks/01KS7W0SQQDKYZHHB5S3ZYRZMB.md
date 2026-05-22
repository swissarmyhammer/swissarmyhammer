---
assignees:
- claude-code
depends_on:
- 01KS865E8GE912VE7KSW836N8V
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff9e80
project: ai-panel
title: 'llama-agent: bridge MCP elicitation to the ACP client (elicitation/create)'
---
#elicitation #llama-agent

## Context / Why
The local-llama backend (`crates/llama-agent`) is an in-process Rust ACP **Agent** whose own MCP client owns the connection to the per-board SAH MCP server (`mcpUrl` → `MCPServerConfig::Http`). For elicitation parity with the Claude backend, llama-agent must (a) advertise the elicitation capability to the MCP server so the server's `peer.create_elicitation(...)` is allowed, and (b) when the SAH server sends an `elicitation/create` during a tool call, redirect it to the user by emitting an ACP `elicitation/create` request to the webview, then return the user's response to the MCP server.

This is the in-process Rust analog of the claude-agent task; here we control both the MCP client and the ACP agent side, so the whole round-trip lives in this crate.

## Dependency
Requires the ACP elicitation types — depends on "Enable ACP elicitation types in the workspace". Uses `agent_client_protocol::schema::{CreateElicitationRequest, CreateElicitationResponse, ElicitationAction}`. The MCP side uses rmcp's elicitation types (mirrors `crates/swissarmyhammer-tools/src/mcp/tools/questions/ask/mod.rs`, the sender). ACP method name is `elicitation/create`.

## Investigation findings (2026-05-22) — subtask 1 complete
Verified versions: `agent-client-protocol` **0.11.1**, `agent-client-protocol-schema` **0.12.0** (unstable_elicitation on), `rmcp` **1.5.0** (workspace enables `elicitation` + `server` + `macros`).

- **MCP client setup**: `acp/mcp_client_factory.rs::create_mcp_client_from_acp` builds the per-server `UnifiedMCPClient` and passes it an `Arc<NotifyingClientHandler>` (Http/Sse). The handler is constructed per-session in `acp/server.rs::create_session`.
- **MCP server→client request hook**: `mcp_client_handler.rs::NotifyingClientHandler` implements rmcp `ClientHandler`. rmcp 1.5.0 `ClientHandler::create_elicitation(&self, CreateElicitationRequestParams, RequestContext<RoleClient>) -> Result<CreateElicitationResult, McpError>` is the inbound hook (default Declines).
- **Advertising the capability**: `get_info` now returns `ClientCapabilities::builder().enable_elicitation().build()`.
- **ACP cx access**: the agent's `ConnectionTo<Client>` (`cx`) is only handed to the `connect_with` closure in `server.rs::start_with_streams`. `ConnectionTo` is `Clone`. The bridge captures `cx.clone()`, wraps it in a `ConnectionElicitationSender`, and publishes it into a server-level `ElicitationEndpoint` (`Arc<RwLock<Option<Arc<dyn ElicitationSender>>>>`) shared with every per-session handler; cleared when the connection's bridge loop exits.
- **Sending the ACP request (CRITICAL)**: `agent-client-protocol` 0.11.1 has NO elicitation in its runtime — `CreateElicitationRequest`/`CreateElicitationResponse` do NOT implement 0.11.1's `JsonRpcRequest`/`JsonRpcResponse`, so `cx.send_request(CreateElicitationRequest).block_task()` does NOT compile (confirmed by the parallel claude-agent breakage). Use the extension/raw path: `UntypedMessage::new("elicitation/create", &acp_request)` (impls `JsonRpcRequest` with `Response = serde_json::Value`), `cx.send_request(untyped).block_task().await`, then `serde_json::from_value::<CreateElicitationResponse>`.
- **Translation**: rmcp `CreateElicitationRequestParams::FormElicitationParams { message, requested_schema }` → ACP `CreateElicitationRequest::new(ElicitationFormMode::new(ElicitationSessionScope::new(session_id), acp_schema), message)`. rmcp `ElicitationSchema` → ACP `ElicitationSchema` via JSON round-trip (identical JSON-Schema object shape). URL-mode rmcp requests render as a form whose message carries the URL (local webview collects inline). ACP response → rmcp result: Accept{content} → result Accept with JSON content; Decline/Cancel map straight across.
- **Session/tool_call context**: handler tracks `current_session: Arc<Mutex<Option<SessionId>>>`; reused for `ElicitationSessionScope.session_id`. `tool_call_id` not threaded into the handler — `None` for now (form mode with session scope), acceptable per task ("with tool_call_id when known").
- **Graceful fallback**: no endpoint (no ACP client) OR no session context → decline.

## Implementation summary
- New module `crates/llama-agent/src/acp/elicitation.rs`:
  - `ElicitationSender` trait + production `ConnectionElicitationSender` (wraps `ConnectionTo<Client>`, sends via `UntypedMessage`).
  - Pure translation: `mcp_request_to_acp`, `acp_response_to_mcp`, schema/content helpers.
  - `bridge_elicitation(sender, params, session_id, client_supports_elicitation)` — full round-trip with decline fallback.
- `crates/llama-agent/src/mcp_client_handler.rs`:
  - `NotifyingClientHandler` gains a shared `ElicitationEndpoint` and a shared `SharedClientCapabilities`; `with_elicitation_endpoint` constructor; `new` keeps empty slots.
  - `get_info` advertises elicitation; `create_elicitation` delegates to testable `relay_elicitation`; `client_supports_elicitation` gates on the advertised capability.
- `crates/llama-agent/src/acp/server.rs`:
  - `AcpServer` gains `elicitation_endpoint`; per-session handlers share it plus the existing `client_capabilities`; `connect_with` publishes the live `ConnectionElicitationSender` on connect and clears it on disconnect.

## What
- [x] **Investigate first (subtask):** Located MCP client setup, server→client request hook, and ACP `cx` reachability. Documented above.
- [x] Make the MCP client advertise elicitation capability in its MCP `initialize` (`get_info` → `enable_elicitation()`).
- [x] On an inbound MCP `elicitation/create`, translate to an ACP `CreateElicitationRequest` (form mode, session scope) and send to the webview via the live connection; await the `CreateElicitationResponse`.
- [x] Translate the ACP response (`ElicitationAction` accept{content}/decline/cancel) back into the rmcp elicitation result returned to the SAH MCP server.
- [x] Graceful fallback (decline) if no ACP client is connected (or no session context).

## Acceptance Criteria
- [x] llama-agent's MCP client advertises elicitation capability. (`get_info_advertises_elicitation_capability` test)
- [x] An MCP `elicitation/create` produces one ACP `elicitation/create` to the client and relays the response back to the MCP server. (`relay_emits_one_acp_request_and_round_trips_accept`)
- [x] Decline/cancel propagate as the correct rmcp elicitation actions. (`decline_*`/`cancel_*`/`relay_propagates_cancel`)

## Tests (`crates/llama-agent/...`)
- [x] Integration test with a fake ACP client + a stub MCP elicitation: asserts the ACP `elicitation/create` request is emitted and the response round-trips to the MCP layer (accept/decline/cancel). 10 new tests across `acp::elicitation` and `mcp_client_handler`, written TDD (RED stubs observed failing, then GREEN).
- [x] Run: `cargo nextest run -p llama-agent` — 1096 tests, all green. `cargo clippy -p llama-agent --all-targets` clean.

## Workflow
- Used `/tdd`: stubbed the response/capability paths, watched the accept/cancel/capability tests fail for the right reason, then implemented and confirmed green.

## Review Findings (2026-05-22 17:35)

### Nits
- [x] `crates/llama-agent/src/acp/elicitation.rs` (`bridge_elicitation`) — Parity gap with the claude-agent bridge: `bridge_elicitation` gated only on (sender present, session present) and did not verify the connected ACP client advertised the `elicitation` capability before sending, whereas `claude-agent`'s `ElicitationBridgeHandler::client_supports_elicitation` declines first when the capability is absent. **Resolved:** threaded the shared `client_capabilities` cell into the handler for behavioral parity. `NotifyingClientHandler` now holds a `SharedClientCapabilities` (`Arc<RwLock<Option<ClientCapabilities>>>`) shared with `AcpServer` — the same cell already populated from the `initialize` request. Added `NotifyingClientHandler::client_supports_elicitation` (mirrors claude-agent's helper) and a `client_supports_elicitation: bool` gate to `bridge_elicitation`, which declines up front (without contacting the client) when the capability is absent. `with_elicitation_endpoint` takes the capabilities cell; `new` defaults it empty. Wired at the handler-creation site in `server.rs` via `self.client_capabilities.clone()`. Added 2 tests (`acp::elicitation::unsupported_capability_declines_without_sending`, `mcp_client_handler::relay_without_capability_declines_without_sending`) asserting decline-without-send; `relay_without_endpoint_declines` now isolates the no-endpoint path (caps present). `cargo nextest run -p llama-agent` 1096 passed; `cargo clippy -p llama-agent --all-targets` clean.
