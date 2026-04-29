---
assignees:
- claude-code
depends_on:
- 01KQD0D883ZW5JAA02913DXM8E
position_column: todo
position_ordinal: fb80
project: acp-upgrade
title: Adapt ACP consumers (swissarmyhammer-agent + drop swissarmyhammer-tools dep)
---
## What

Adapt the lighter ACP consumers to the new ACP 0.11.1 SDK design. See spike findings on task 01KQ367HE0Z8ZSXY90CTT8QYGG and migration guide https://agentclientprotocol.github.io/rust-sdk/migration_v0.11.x.html.

> **Post-merge update (2026-04-29)**: `avp-common` previously included here is now an Agent implementor (production + mock `impl Agent`) and has been moved into the atomic rewrite task `01KQ7KP7HVASAD4V45AJ41P39W`. Only `swissarmyhammer-agent` and `swissarmyhammer-tools` remain in scope here.

## Spike-confirmed scope

### `swissarmyhammer-tools` — drop the dep entirely

Re-confirmed post-merge: `grep agent_client_protocol swissarmyhammer-tools/src/` returns **zero** matches even though avp added ~1500 lines of new files there. The only mention of `agent-client-protocol` is the `agent-client-protocol = { workspace = true }` line in `swissarmyhammer-tools/Cargo.toml` (line 120). The dep is dead.

**Action**: delete the `agent-client-protocol = { workspace = true }` line from `swissarmyhammer-tools/Cargo.toml`. No source changes.

### `swissarmyhammer-agent`

47 lines reference `agent_client_protocol` (unchanged from spike). Imports include `Agent`, `ClientCapabilities`, `ContentChunk`, `FileSystemCapabilities`, `ImageContent`, `McpServer`/`McpServerHttp`/`McpServerStdio`, `PromptResponse`, `ProtocolVersion::V1`, `SessionId`, `ToolCall`, `ToolCallId`, plus `agent_client_protocol_extras::{trace_notifications, TracingAgent}`.

Adaptation:
- Schema-type imports: `agent_client_protocol::X` → `agent_client_protocol::schema::X` for all 12 schema types listed above.
- `Agent` consumption: depends on whatever new wrapper extras exposes. If extras settles on a `TracingAgent: <new abstraction>` wrapper, this crate just consumes the new shape. The actual surface here is small — mostly client-side construction of `McpServer*`, `ClientCapabilities`, etc.
- `trace_notifications` and `TracingAgent` imports continue to come from `agent_client_protocol_extras` (extras crate handles the rewrite).

## Acceptance Criteria
- [ ] `agent-client-protocol = { workspace = true }` is removed from `swissarmyhammer-tools/Cargo.toml`.
- [ ] `cargo check -p swissarmyhammer-agent --all-targets` passes.
- [ ] `cargo check -p swissarmyhammer-tools --all-targets` passes.
- [ ] `cargo clippy -p swissarmyhammer-agent -p swissarmyhammer-tools --all-targets -- -D warnings` passes.

## Tests
- [ ] `cargo nextest run -p swissarmyhammer-agent` — green.
- [ ] `cargo nextest run -p swissarmyhammer-tools` — green.

## Workflow
- Adaptation, not rewrite — `swissarmyhammer-agent` is a downstream consumer (not an Agent implementor); `swissarmyhammer-tools` only needs the dead dep removed. Once the rewrite task lands, the ripple here is small.
- Verify CLI surface in `swissarmyhammer-cli/src/commands/agent/acp.rs` is unaffected (the spike grep found zero direct ACP usage in `swissarmyhammer-cli/src/`; treat as covered by the swissarmyhammer-agent edits).

## Depends on
- 01KQ7KP7HVASAD4V45AJ41P39W (the atomic SDK-rewrite task — extras + claude-agent + llama-agent + avp-common all in place).
- Spike findings: 01KQ367HE0Z8ZSXY90CTT8QYGG.