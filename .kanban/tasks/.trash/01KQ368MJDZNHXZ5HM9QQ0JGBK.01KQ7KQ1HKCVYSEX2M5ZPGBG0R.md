---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: f780
project: acp-upgrade
title: Adapt agent-client-protocol-extras to ACP 0.11
---
## What

**REWRITE** `agent-client-protocol-extras/` against the new ACP 0.11.1 SDK design. This is not a "fix imports + add match arms" adaptation — the underlying `Agent` trait was removed in 0.11.0 and replaced by a builder/handler pattern (see migration guide: https://agentclientprotocol.github.io/rust-sdk/migration_v0.11.x.html and the spike findings on task 01KQ367HE0Z8ZSXY90CTT8QYGG).

## Spike-confirmed scope

- **3 `impl Agent for X` blocks** must be replaced with the new wrapper shape: `HookableAgent` (`hookable_agent.rs`), `TracingAgent` (`tracing_agent.rs`), `RecordingAgent`/`PlaybackAgent` (`recording.rs`, `playback.rs`).
- **`Arc<dyn Agent + Send + Sync>` field types** in HookableAgent and TracingAgent are no longer meaningful (no trait → no trait object). Replace with whatever 0.11 wrapper abstraction we settle on (likely a builder-stage hook or a `Dispatch` interceptor).
- **`AgentWithFixture: Agent` supertrait bound** in `lib.rs` is broken. Decide whether AgentWithFixture remains a marker trait or becomes a concrete wrapper type.
- **23 schema-type imports** across `hookable_agent.rs`, `tracing_agent.rs`, `recording.rs`, `playback.rs`, `hook_config.rs` need to move from `agent_client_protocol::X` to `agent_client_protocol::schema::X`. Types affected: `AuthenticateRequest`, `AuthenticateResponse`, `CancelNotification`, `ContentBlock`, `ContentChunk`, `ExtNotification`, `ExtRequest`, `ExtResponse`, `InitializeRequest`, `InitializeResponse`, `LoadSessionRequest`, `LoadSessionResponse`, `NewSessionRequest`, `NewSessionResponse`, `PromptRequest`, `PromptResponse`, `SessionNotification`, `SessionUpdate`, `SetSessionModeRequest`, `SetSessionModeResponse`, `StopReason`, `TextContent`, `ToolCallStatus`. Also: `AvailableCommandsUpdate`, `CurrentModeUpdate`, `Plan`, `ToolCall`, `ToolCallUpdate`, `ToolCallUpdateFields`, `SessionId`, `ProtocolVersion::LATEST`, `ErrorCode::InvalidRequest`, `Result`, `PromptResponse`, `Error::{internal_error, method_not_found, new}`.
- **`Error::auth_methods` was removed in 0.11.0**. Audit `hook_config.rs` error-construction.
- **`agent-client-protocol-extras/tests/e2e_hooks/*.rs`** will need refresh as the public API of HookableAgent shifts.

## Acceptance Criteria
- [ ] `cargo check -p agent-client-protocol-extras --all-targets` passes.
- [ ] `cargo clippy -p agent-client-protocol-extras --all-targets -- -D warnings` passes.
- [ ] No silent `_` catch-alls added on enum matches that previously enumerated cases — handle new variants explicitly or document why we don't.
- [ ] The new wrapper shape for HookableAgent/TracingAgent/RecordingAgent/PlaybackAgent is documented in module-level doc comments — explain the migration from `impl Agent` to the new pattern.

## Tests
- [ ] `cargo nextest run -p agent-client-protocol-extras` — all pass.
- [ ] If any test fixtures (recorded sessions in `acp-conformance/.fixtures/*`) were touched by recording-format changes, regenerate via the conformance crate (covered separately by the conformance task).

## Workflow
- Read the migration guide first: https://agentclientprotocol.github.io/rust-sdk/migration_v0.11.x.html
- Read `examples/simple_agent.rs` in the 0.11.1 source for the canonical builder pattern.
- Use `/tdd` for any new behavior; otherwise this is a **rewrite** — keep behavior parity with the existing test suite.
- Sequence: get extras compiling and unit-tested first, then unblock claude-agent + llama-agent to adapt against the new wrapper API.

## Depends on
- 01KQ367XFMW2CP7GWM4GJ41BNR (version bump landed).
- Spike findings: 01KQ367HE0Z8ZSXY90CTT8QYGG.
