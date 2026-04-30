---
assignees:
- claude-code
position_column: todo
position_ordinal: fe80
project: acp-upgrade
title: Rebuild AgentWithFixture + fixture helpers + TestMcpServer in agent-client-protocol-extras (ACP 0.11)
---
## What

The acp-conformance crate (task 01KQ36AGXFCJF4PEEK2TDN6YQK) and other downstream tests depend on a set of fixture-recording helpers in `agent-client-protocol-extras` that existed in 0.10 but were never re-implemented under the 0.11 rewrite.

The 0.11 lib.rs even calls them out explicitly:

> The `test_mcp_server` module is migrated by a sibling task (D-series) and is not yet wired in here. The fixture-recording entry points (`with_fixture`, `AgentWithFixture`, `start_test_mcp_server_with_capture`) are likewise rebuilt by those tasks.

But none of A1–A5 / B0–B10 / C1–C11 / D1–D4 actually rebuilt them.

## Surfaces to (re)build in `agent-client-protocol-extras`

### 1. `AgentWithFixture` abstraction

In 0.10 this was `trait AgentWithFixture: Agent { fn agent_type() -> &'static str; }`. In 0.11 there is no `Agent` trait, so the abstraction must be redesigned around `ConnectionTo<Agent>`.

Recommended shape:

```rust
// boxed, dyn-compatible. Tests get one of these from a factory and pass it
// to conformance scenario functions that call `agent.connection().send_request(...)`.
pub trait AgentWithFixture: Send + Sync {
    fn agent_type(&self) -> &'static str;
    fn connection(&self) -> ConnectionTo<Agent>;
}
```

The connection field is owned long enough to outlive every conformance call (the recording flush happens on drop of the wrapper, which closes the channel and lets the inner agent's `connect_to(...)` future resolve).

A concrete `PlaybackAgentWithFixture` and `RecordingAgentWithFixture` should be provided so the per-agent factories in `acp-conformance/tests/common/mod.rs` can return `Box<dyn AgentWithFixture>`.

### 2. `get_fixture_path_for(agent_type, test_name) -> PathBuf`

Returns `<workspace>/.fixtures/<agent_type>/<test_name>.json`. Used by both the conformance src (verifier helpers) and the test factories.

### 3. `get_test_name_from_thread() -> String`

Reads the current `tokio::test`/`std::thread` thread name and returns the leaf component. Used by the per-agent factories to pick the right fixture file.

### 4. `start_test_mcp_server_with_capture() -> Result<TestMcpServer>`

In 0.10 this started an in-process MCP HTTP server with proxy capture so notifications could be recorded into the fixture. In 0.11 we need to:

- Reuse / port the existing `model-context-protocol-extras::McpProxy` (already public).
- Expose a `TestMcpServer` with `url() -> &str` and `subscribe() -> McpNotificationSource`.

### 5. `RecordingAgent::with_notifications(...)` + `add_mcp_source(...)` + `McpNotificationSource`

The recording wrapper needs to multiplex notifications coming from the wrapped agent **and** from the MCP proxy into a single recorded fixture. The 0.11 `RecordingAgent` only wraps a `ConnectTo<Client>`; this task adds the notification-capture variant.

## Acceptance Criteria

- [ ] `AgentWithFixture` trait exposed from `agent_client_protocol_extras`, with at least `PlaybackAgentWithFixture` (and ideally `RecordingAgentWithFixture`) concrete impls.
- [ ] `get_fixture_path_for` and `get_test_name_from_thread` helpers exposed.
- [ ] `start_test_mcp_server_with_capture` + `TestMcpServer` + `McpNotificationSource` exposed.
- [ ] `RecordingAgent::with_notifications` + `add_mcp_source` API present.
- [ ] `cargo check -p agent-client-protocol-extras --all-targets` clean.
- [ ] `cargo nextest run -p agent-client-protocol-extras` green.

## Tests

- [ ] Unit tests for `get_fixture_path_for` and `get_test_name_from_thread`.
- [ ] An end-to-end test that builds a `PlaybackAgentWithFixture`, calls a couple of `connection.send_request(...)` against it, and verifies fixture deserialise + cursor advancement.
- [ ] An end-to-end test that wires a `RecordingAgent::with_notifications` + `start_test_mcp_server_with_capture` and confirms notifications from both sources are flushed to disk on drop.

## Blocks

This is required by:
- 01KQ36AGXFCJF4PEEK2TDN6YQK (Adapt acp-conformance to ACP 0.11) — the entire conformance test crate uses these helpers in `tests/common/mod.rs` and `tests/integration/*.rs`.

## References

- 0.10 implementation: `git log --all --oneline -- "**/test_mcp_server*"` and `git log --all -p -- agent-client-protocol-extras/src/lib.rs` for the original API.
- Wire patterns from `avp-common/src/validator/runner.rs` `run_with_mock_agent` / `run_with_playback_agent` (D3) — those are the closest 0.11 pattern for adapting an "owned agent" to a `ConnectionTo<Agent>` for the test body to drive.
- `llama-agent/src/acp/server.rs` (C9) — full inherent-method + builder reshape pattern.