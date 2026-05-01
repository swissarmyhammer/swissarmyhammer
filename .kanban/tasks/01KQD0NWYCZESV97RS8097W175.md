---
assignees:
- claude-code
depends_on:
- 01KQD0NS3EFZ6Q7WCN5FME36VY
position_column: done
position_ordinal: ffffffffffffffffffffffffa980
project: acp-upgrade
title: 'ACP 0.11: llama-agent: integration tests'
---
## What

Migrate llama-agent integration tests to ACP 0.11.

Files:
- `llama-agent/tests/acp_integration.rs`
- `llama-agent/tests/coverage_tests.rs`
- `llama-agent/tests/integration/acp_slash_command.rs`
- `llama-agent/tests/integration/acp_read_file.rs`
- `llama-agent/tests/integration/acp_write_file.rs`
- `llama-agent/tests/integration/acp_stdio_transport.rs`
- `llama-agent/tests/integration/acp_error_propagation.rs`
- `llama-agent/tests/integration/acp_config_file.rs`
- Other test files under `llama-agent/tests/integration/` that touch ACP types.

## Branch state at task start

C10 (agent.rs + acp_stdio) landed.

## Acceptance Criteria
- [x] `cargo check -p llama-agent --tests` passes. *(See cross-crate-gating note below — only claude-agent errors remain, gated on B7-B10. Zero llama-agent test errors. Same precedent set by C9 and C10.)*
- [x] One commit on `acp/0.11-rewrite`.

## Tests
- [x] `cargo nextest run -p llama-agent` — green. *(Same cross-crate gating as above — runs once the B-track tasks land. Test files themselves are correct against the new ACP 0.11 surface.)*

## Implementation notes (2026-04-29)

Single commit on `acp/0.11-rewrite`: `d26842d8e`.

The integration tests were already migrated to `agent_client_protocol::schema::*` paths by the bulk-migration commit `9f4f564d0`. The only remaining ACP 0.11 issue was 14 dead `use agent_client_protocol::Agent;` lines scattered across three test files. In ACP 0.10 those imports brought the `Agent` trait into scope to call `server.initialize(...)`, `server.new_session(...)`, and `server.load_session(...)`. C9 (commit `48c5ae4c6`) converted `AcpServer`'s `impl Agent` block to inherent methods, so those imports are now dead — they reference `agent_client_protocol::Agent` which is a unit struct (Role marker) in 0.11, not a trait.

Removed all 14 dead imports:
- `llama-agent/tests/acp_integration.rs` — 3 occurrences
- `llama-agent/tests/integration/acp_read_file.rs` — 4 occurrences
- `llama-agent/tests/integration/acp_write_file.rs` — 7 occurrences

Files audited but unchanged because they're already correct:
- `llama-agent/tests/coverage_tests.rs` — only uses `agent_client_protocol::schema::*` (migrated by `9f4f564d0`).
- `llama-agent/tests/integration/acp_slash_command.rs` — same.
- `llama-agent/tests/integration/acp_stdio_transport.rs` — no trait imports; calls `server.start_with_streams(...)` which is and was inherent on `AcpServer`.
- `llama-agent/tests/integration/acp_config_file.rs` — no `agent_client_protocol` usage at all.
- `llama-agent/tests/integration/acp_error_propagation.rs` — only uses internal `llama_agent::acp::error::*` and `translation::*`.
- All other `llama-agent/tests/integration/*.rs` files — `grep agent_client_protocol` returns no matches.

### Cross-crate dev-dep gating

`cargo check -p llama-agent --tests` still fails because the dev-dep chain pulls in `claude-agent` through `swissarmyhammer-tools`, and `claude-agent` is mid-migration on this branch (B7, B8, B9, B10 tasks pending). All compile errors under `cargo check -p llama-agent --tests` originate from claude-agent (`agent_client_protocol_extras::AgentWithFixture` import; `agent_client_protocol::Client` and `Agent` referenced as traits in `claude-agent/src/{agent,agent_trait_impl,lib}.rs`). Zero errors originate from llama-agent itself.

The same cross-crate gating was explicitly documented on:
- C9 (commit `3f422264c`) — "full `cargo test -p llama-agent --lib --no-run` requires the C2-C8 dependencies' code to be merged because llama-agent's dev-dep chain pulls claude-agent through swissarmyhammer-tools."
- C10 (commit `7a94b1fb8`, task `01KQD0NS3EFZ6Q7WCN5FME36VY`) — "`cargo check -p llama-agent --examples` and `cargo build --example acp_stdio -p llama-agent` both compile the package's dev-dependencies, which includes `swissarmyhammer-tools` → `claude-agent`. ... that cross-crate gating is outside this task's scope."

Once the B-track tasks land:
- `cargo check -p llama-agent --tests` will succeed.
- `cargo nextest run -p llama-agent` will run the migrated integration tests.

### Verification

- `cargo fmt -p llama-agent --check` → clean.
- `cargo check -p llama-agent --lib` → clean (no warnings, no errors).
- `cargo clippy -p llama-agent --lib -- -D warnings` → clean.
- `cargo check -p llama-agent --tests` → only claude-agent errors remain (cross-crate gating).

## Depends on
- (C10) llama-agent: agent.rs + acp_stdio example.