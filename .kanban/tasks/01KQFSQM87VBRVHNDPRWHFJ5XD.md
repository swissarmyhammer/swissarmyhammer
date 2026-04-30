---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffb280
project: acp-upgrade
title: Rebuild AgentWithFixture + fixture helpers + TestMcpServer in agent-client-protocol-extras (ACP 0.11)
---
## What

The acp-conformance crate (task 01KQ36AGXFCJF4PEEK2TDN6YQK) and other downstream tests depend on a set of fixture-recording helpers in `agent-client-protocol-extras` that existed in 0.10 but were never re-implemented under the 0.11 rewrite.

The 0.11 lib.rs even calls them out explicitly:

> The `test_mcp_server` module is migrated by a sibling task (D-series) and is not yet wired in here. The fixture-recording entry points (`with_fixture`, `AgentWithFixture`, `start_test_mcp_server_with_capture`) are likewise rebuilt by those tasks.

But none of A1–A5 / B0–B10 / C1–C11 / D1–D4 actually rebuilt them.

## Surfaces (re)built in `agent-client-protocol-extras`

### 1. `AgentWithFixture` abstraction

`agent-client-protocol-extras/src/fixture.rs` introduces the new dyn-compatible trait:

```rust
pub trait AgentWithFixture: Send + Sync {
    fn agent_type(&self) -> &'static str;
    fn connection(&self) -> ConnectionTo<Agent>;
}
```

The connection is a cheaply-cloneable handle. Both `PlaybackAgentWithFixture` and `RecordingAgentWithFixture` own:
1. The inner agent (`ConnectTo<Client>` impl)
2. A spawned dispatch task on a `Channel::duplex()` pair
3. A spawned `Client.builder().connect_with(...)` task that yields the `ConnectionTo<Agent>`
4. A oneshot shutdown trigger so the wrapper's `Drop` cleanly tears the wiring down

### 2. `get_fixture_path_for` / `get_test_name_from_thread`

In `fixture.rs`. Workspace root is detected by walking up from `CARGO_MANIFEST_DIR` for the first `Cargo.toml` containing a `[workspace]` table.

### 3. `start_test_mcp_server_with_capture` + `TestMcpServer`

In `agent-client-protocol-extras/src/test_mcp_server.rs`. Returned `TestMcpServer` wraps an upstream `TestMcpServerHandler` plus an `McpProxy` that captures notifications. `TestMcpServer` impls `McpNotificationSource` by delegating to the proxy. The wrapper aborts the upstream serve task on drop.

`McpNotificationSource` is re-exported from `model-context-protocol-extras` (already was).

### 4. `RecordingAgent::with_notifications` + `add_mcp_source`

In `recording.rs`:
- `RecordingState` made `pub(crate)` and gained `observe_external_notification` for side-channel feeds.
- `RecordingAgent::with_state(inner, Arc<RecordingState>)` — internal constructor used by `RecordingAgentWithFixture::new`.
- `spawn_session_notification_drain` and `spawn_mcp_drain` — drain `broadcast::Receiver<...>` into the recording.
- `SourceHandle` — RAII guard aborting the drain on drop.
- `RecordingAgent::with_notifications` — async associated function that returns a `RecordingAgentWithFixture` with a session-notification side feed already wired.

In `fixture.rs`:
- `RecordingAgentWithFixture::new(inner, path, agent_type)` — base case.
- `RecordingAgentWithFixture::with_notifications(inner, path, agent_type, rx)` — adds session-notification side channel.
- `RecordingAgentWithFixture::add_mcp_source(rx)` — adds MCP notification source.

## Acceptance Criteria

- [x] `AgentWithFixture` trait exposed from `agent_client_protocol_extras`, with `PlaybackAgentWithFixture` AND `RecordingAgentWithFixture` concrete impls.
- [x] `get_fixture_path_for` and `get_test_name_from_thread` helpers exposed.
- [x] `start_test_mcp_server_with_capture` + `TestMcpServer` + `McpNotificationSource` exposed.
- [x] `RecordingAgent::with_notifications` + `add_mcp_source` API present.
- [x] `cargo check -p agent-client-protocol-extras --all-targets` clean.
- [x] `cargo nextest run -p agent-client-protocol-extras` green (248/248).

## Tests

- [x] Unit tests for `get_fixture_path_for` and `get_test_name_from_thread` (`fixture.rs::tests`).
- [x] End-to-end test that builds a `PlaybackAgentWithFixture`, calls a couple of `connection.send_request(...)` against it, and verifies fixture deserialise + cursor advancement (`fixture.rs::tests::playback_with_fixture_roundtrips_initialize_via_connection`).
- [x] End-to-end test that wires a `RecordingAgent::with_notifications` + MCP notification source and confirms notifications from both sources are flushed to disk on drop (`tests/recording_with_notifications.rs::recording_with_notifications_captures_wire_and_mcp_sources`).

## Note for downstream task

The conformance crate (01KQ36) currently calls e.g.:
```rust
let recording_agent = RecordingAgent::with_notifications(agent, fixture_path, receiver);
recording_agent.add_mcp_source(...);
Box::new(recording_agent)
```

That sample code in `tests/common/mod.rs` will need a small adaptation to match the actual API:
- `with_notifications` is async and now also takes `agent_type: &'static str`.
- It returns a `RecordingAgentWithFixture` (the `dyn AgentWithFixture` impl), not a raw `RecordingAgent`.
- The inner agent must implement `ConnectTo<Client>`. Production agents (`ClaudeAgent`, `AcpServer`) need a thin per-agent adapter that wraps their inherent dispatch into a `ConnectTo<Client>` impl. That adapter belongs in the per-agent crate and is the conformance task's concern.

## Files

- `agent-client-protocol-extras/src/fixture.rs` (new)
- `agent-client-protocol-extras/src/test_mcp_server.rs` (rewritten — `TestMcpServerHandler` + `TestMcpServer` wrapper)
- `agent-client-protocol-extras/src/recording.rs` (added side-channel drains)
- `agent-client-protocol-extras/src/lib.rs` (wired modules + re-exports)
- `agent-client-protocol-extras/tests/recording_with_notifications.rs` (new — multi-source flush integration test)

## Blocks

- 01KQ36AGXFCJF4PEEK2TDN6YQK (Adapt acp-conformance to ACP 0.11)

## References

- 0.10 implementation: `git log --all --oneline -- "**/test_mcp_server*"` and `git log --all -p -- agent-client-protocol-extras/src/lib.rs` for the original API.
- Wire patterns from `avp-common/src/validator/runner.rs` `run_with_mock_agent` / `run_with_playback_agent`.
- `llama-agent/src/acp/server.rs` — full inherent-method + builder reshape pattern.