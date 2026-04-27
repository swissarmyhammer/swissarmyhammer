---
assignees:
- claude-code
depends_on:
- 01KQ7KP7HVASAD4V45AJ41P39W
position_column: todo
position_ordinal: fb80
project: acp-upgrade
title: Adapt ACP consumers (swissarmyhammer-agent, swissarmyhammer-tools, avp-common)
---
## What

Adapt the lighter ACP consumers (`swissarmyhammer-agent`, `swissarmyhammer-tools`, `avp-common`) to the new ACP 0.11.1 SDK design. See spike findings on task 01KQ367HE0Z8ZSXY90CTT8QYGG and migration guide https://agentclientprotocol.github.io/rust-sdk/migration_v0.11.x.html.

## Spike-confirmed scope

### `swissarmyhammer-tools` — drop the dep entirely

The spike confirmed: `grep -r "agent_client_protocol" swissarmyhammer-tools/src/` returns **zero** matches. The only mention is `agent-client-protocol = { workspace = true }` in `swissarmyhammer-tools/Cargo.toml` (line 120). The dep is dead.

**Action**: delete the `agent-client-protocol = { workspace = true }` line from `swissarmyhammer-tools/Cargo.toml`. No source changes.

### `swissarmyhammer-agent`

47 lines reference `agent_client_protocol`. Imports include `Agent`, `ClientCapabilities`, `ContentChunk`, `FileSystemCapabilities`, `ImageContent`, `McpServer`/`McpServerHttp`/`McpServerStdio`, `PromptResponse`, `ProtocolVersion::V1`, `SessionId`, `ToolCall`, `ToolCallId`, plus `agent_client_protocol_extras::{trace_notifications, TracingAgent}`.

Adaptation:
- Schema-type imports: `agent_client_protocol::X` → `agent_client_protocol::schema::X` for all 12 schema types listed above.
- `Agent` consumption: depends on whatever new wrapper extras exposes. If extras settles on a `TracingAgent: <new abstraction>` wrapper, this crate just consumes the new shape. The actual surface here is small — mostly client-side construction of `McpServer*`, `ClientCapabilities`, etc.
- `trace_notifications` and `TracingAgent` imports continue to come from `agent_client_protocol_extras` (extras crate handles the rewrite).

### `avp-common`

8 lines reference `agent_client_protocol`. Imports: `Agent` (in `src/context.rs` and `src/validator/runner.rs`), `SessionNotification` (same files), `StopReason` (in `src/validator/executor.rs`), plus `agent_client_protocol_extras::PlaybackAgent` in tests.

Adaptation:
- `agent_client_protocol::SessionNotification` → `agent_client_protocol::schema::SessionNotification`
- `agent_client_protocol::StopReason` → `agent_client_protocol::schema::StopReason`
- `Agent` consumption depends on extras' new wrapper API.
- The two `StopReason::EndTurn` constructions (`src/validator/executor.rs:916`, line 2135) are unchanged in shape; just import path moves.
- `PlaybackAgent` is re-exported by extras.

## Acceptance Criteria
- [ ] `agent-client-protocol = { workspace = true }` is removed from `swissarmyhammer-tools/Cargo.toml`.
- [ ] `cargo check -p swissarmyhammer-agent --all-targets` passes.
- [ ] `cargo check -p swissarmyhammer-tools --all-targets` passes.
- [ ] `cargo check -p avp-common --all-targets` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes for all three.

## Tests
- [ ] `cargo nextest run -p swissarmyhammer-agent` — green.
- [ ] `cargo nextest run -p swissarmyhammer-tools` — green.
- [ ] `cargo nextest run -p avp-common` — green.

## Workflow
- Adaptation, not rewrite — these crates are downstream consumers, not Agent implementors. Once the rewrite task lands, the ripple here is small.
- Verify CLI surface in `swissarmyhammer-cli/src/commands/agent/acp.rs` is unaffected (the spike grep found zero direct ACP usage in `swissarmyhammer-cli/src/`; treat as covered by the swissarmyhammer-agent edits).

## Depends on
- 01KQ7KP7HVASAD4V45AJ41P39W (the atomic SDK-rewrite task — extras + claude-agent + llama-agent must all be in place).
- Spike findings: 01KQ367HE0Z8ZSXY90CTT8QYGG.