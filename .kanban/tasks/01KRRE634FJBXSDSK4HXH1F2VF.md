---
assignees:
- claude-code
depends_on:
- 01KRRE4PJV0N1GSE92MF45GGPV
position_column: todo
position_ordinal: '8980'
project: plugin-arch
title: 'plugin: UrlServer HTTP transport'
---
## What
Implement `UrlServer` — an `McpServer` backed by an HTTP-served MCP endpoint.

In `crates/swissarmyhammer-plugin/src/server/url.rs`:
- Constructed from `{ url: String, headers: Option<Map<String,String>> }` (the URL `ServerSource` variant).
- On connect: perform the MCP `initialize` handshake over HTTP, send `tools/list`, cache the response as `Vec<ToolMetadata>`. Subscribe/poll for `notifications/tools/list_changed` to refresh.
- `invoke(caller, tool, input)` → send a JSON-RPC `tools/call` to the configured URL (tool name + arguments map, unchanged); reuse the registration's auth headers on every request; await and return the result `Value`.
- **Build on the rmcp client SDK** — use rmcp's HTTP / streamable-HTTP client transport as the foundation; fall back to `reqwest` JSON-RPC framing only for gaps rmcp does not cover. Connection errors map to platform `Error` (`ServerUnavailable` for transport failure).

## Acceptance Criteria
- [ ] `UrlServer` exists, implements `McpServer`, completes the MCP handshake over HTTP via the rmcp client.
- [ ] `tools()` reflects the endpoint's `tools/list`; `invoke` round-trips a `tools/call` over HTTP with the configured headers.
- [ ] Transport failure surfaces as a clean platform `Error`, not a panic.

## Tests
- [ ] Integration test in `swissarmyhammer-plugin/tests/`: stand up a mock HTTP MCP endpoint (e.g. an in-process `axum`/`wiremock` server, or a real rmcp HTTP server) that records the request shape. Register a `UrlServer`, call a tool, assert the recorded request carried the right tool name, arguments map, and auth header, and that the response `Value` came back.
- [ ] Test that an unreachable URL yields `Err(ServerUnavailable)`.
- [ ] Run: `cargo test -p swissarmyhammer-plugin` — all green.

## Workflow
- Use `/tdd` — write the mock-endpoint round-trip test first, then implement.

## Depends on
McpServer trait + ToolMetadata.