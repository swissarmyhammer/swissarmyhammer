---
assignees:
- claude-code
depends_on:
- 01KQD0D883ZW5JAA02913DXM8E
- 01KQD0MMR7W64307S03XBV69BH
- 01KQD0NS3EFZ6Q7WCN5FME36VY
- 01KQD0KW8SMGT4YYCNH7QN0ANQ
position_column: todo
position_ordinal: ffa080
project: acp-upgrade
title: Adapt ACP consumers (swissarmyhammer-agent + drop swissarmyhammer-tools dep)
---
## What

Adapt the lighter ACP consumers to the new ACP 0.11.1 SDK design. See spike findings on task 01KQ367HE0Z8ZSXY90CTT8QYGG and migration guide https://agentclientprotocol.github.io/rust-sdk/migration_v0.11.x.html.

> **Post-merge update (2026-04-29)**: `avp-common` previously included here is now an Agent implementor (production + mock `impl Agent`) and has been moved into the atomic rewrite task `01KQ7KP7HVASAD4V45AJ41P39W`. Only `swissarmyhammer-agent` and `swissarmyhammer-tools` remain in scope here.

## Spike-confirmed scope

### `swissarmyhammer-tools` — drop the dep entirely

Re-confirmed post-merge: `grep agent_client_protocol swissarmyhammer-tools/src/` returns **zero** matches even though avp added ~1500 lines of new files there. The only mention of `agent-client-protocol` is the `agent-client-protocol = { workspace = true }` line in `swissarmyhammer-tools/Cargo.toml`. The dep is dead.

**Action**: delete the `agent-client-protocol = { workspace = true }` line from `swissarmyhammer-tools/Cargo.toml`. No source changes.

### `swissarmyhammer-agent`

47 lines reference `agent_client_protocol` (unchanged from spike). Imports include `Agent`, `ClientCapabilities`, `ContentChunk`, `FileSystemCapabilities`, `ImageContent`, `McpServer`/`McpServerHttp`/`McpServerStdio`, `PromptResponse`, `ProtocolVersion::V1`, `SessionId`, `ToolCall`, `ToolCallId`, plus `agent_client_protocol_extras::{trace_notifications, TracingAgent}`.

**Note**: `swissarmyhammer-agent/src/lib.rs` line 239 declares `pub agent: Arc<dyn Agent + Send + Sync>`. In ACP 0.11 `Agent` is a unit Role marker (not a trait), so this whole abstraction must be redesigned. The new shape will likely store an `agent_client_protocol::ConnectionTo<Agent>` (or similar) — to be determined by what claude-agent and llama-agent expose after their reshapes (B9, C10).

Adaptation:
- Schema-type imports: `agent_client_protocol::X` → `agent_client_protocol::schema::X` for all 12 schema types listed above.
- `Agent` consumption: redesign `AcpAgentHandle` and `execute_prompt` around the new `ConnectionTo<Agent>` / handler shape, matching whatever claude-agent and llama-agent expose post-reshape.
- `trace_notifications` and `TracingAgent` imports continue to come from `agent_client_protocol_extras` (already migrated by A1).

## Acceptance Criteria
- [ ] `agent-client-protocol = { workspace = true }` is removed from `swissarmyhammer-tools/Cargo.toml`.
- [ ] `cargo check -p swissarmyhammer-agent --all-targets` passes.
- [ ] `cargo check -p swissarmyhammer-tools --all-targets` passes.
- [ ] `cargo clippy -p swissarmyhammer-agent -p swissarmyhammer-tools --all-targets -- -D warnings` passes.

## Tests
- [ ] `cargo nextest run -p swissarmyhammer-agent` — green.
- [ ] `cargo nextest run -p swissarmyhammer-tools` — green.

## Workflow
- Adaptation + redesign — `swissarmyhammer-agent` keeps its public surface (`create_agent`, `execute_prompt`, `AcpAgentHandle`) but the internal `Arc<dyn Agent>` storage must be replaced with whatever the new claude-agent / llama-agent reshapes expose. Don't start until those land.
- `swissarmyhammer-tools` is a one-line Cargo.toml edit — no source changes.
- Verify CLI surface in `swissarmyhammer-cli/src/commands/agent/acp.rs` is unaffected (the spike grep found zero direct ACP usage in `swissarmyhammer-cli/src/`; treat as covered by the swissarmyhammer-agent edits).

## Depends on
- 01KQD0D883ZW5JAA02913DXM8E (A1: extras TracingAgent — already done).
- 01KQD0MMR7W64307S03XBV69BH (B9: claude-agent agent.rs + lib.rs final integration — must land first because swissarmyhammer-agent stores the agent handle).
- 01KQD0NS3EFZ6Q7WCN5FME36VY (C10: llama-agent agent.rs + acp_stdio — same reason for the llama path).
- 01KQD0KW8SMGT4YYCNH7QN0ANQ (D2: avp-common context.rs production Agent reshape — redesign of `Arc<dyn Agent>` should align across all consumers).
- Spike findings: 01KQ367HE0Z8ZSXY90CTT8QYGG.