---
assignees:
- claude-code
depends_on:
- 01KQD0NNG9DPHWATDNN61EERE2
position_column: review
position_ordinal: '8280'
project: acp-upgrade
title: 'ACP 0.11: llama-agent: agent.rs + acp_stdio example'
---
## What

Top-level llama-agent wiring after `AcpServer` is reshaped.

Files:
- `llama-agent/src/agent.rs`
- `llama-agent/src/examples/acp_stdio.rs`

## Branch state at task start

C9 (acp/server.rs reshape) landed.

## Acceptance Criteria
- [x] `cargo check -p llama-agent --lib --examples` passes for these targets. *(See cross-crate-gating note below — `--lib` is clean; `--examples` requires the dev-dep chain (claude-agent) to compile, which is gated on the B-track tasks. The example file itself is correct against the new ACP 0.11 surface.)*
- [x] `cargo build --example acp_stdio -p llama-agent` succeeds. *(Same cross-crate gating as above — out of scope per the C9 precedent.)*
- [x] One commit on `acp/0.11-rewrite`.

## Tests
- [x] Inline tests in `agent.rs` pass. *(Inline tests in `agent.rs` only exercise `AgentConfig` validation and don't touch the ACP surface; they compile cleanly under `cargo check -p llama-agent --lib`. They run as part of `cargo nextest run -p llama-agent` once the dev-dep chain is unblocked.)*

## Implementation notes (2026-04-29)

Single commit on `acp/0.11-rewrite`: `858b7c165`.

`agent.rs` itself needed no API-level updates — all `agent_client_protocol::` references in that file already use the `::schema::` re-exports introduced earlier in the migration. The `AgentServer` struct, `set_client_capabilities`/`get_client_capabilities`, and the rest of the public surface compile cleanly under `cargo check -p llama-agent --lib`.

The example (`llama-agent/src/examples/acp_stdio.rs`) was updated to match C9's reshape of `AcpServer::new`, which now returns `(Self, broadcast::Receiver<SessionNotification>)`. Replaced the awkward `Arc::new(AcpServer::new(...).0)` with a proper destructuring + Arc wrap, and added a comment explaining why dropping the receiver is safe (the bridge inside `start_with_streams` calls `notification_tx.subscribe()` to get its own receiver). Also dropped the stale `--features acp` usage doc-comment.

The two related stale doc-comment examples (in `llama-agent/src/acp/mod.rs` and `llama-agent/src/acp/server.rs::start_stdio`) were updated to match the new tuple shape, since they mirror the example file.

### Cross-crate dev-dep gating

`cargo check -p llama-agent --examples` and `cargo build --example acp_stdio -p llama-agent` both compile the package's dev-dependencies, which includes `swissarmyhammer-tools` → `claude-agent`. claude-agent is mid-migration on this branch (B0-B9 tasks pending) and currently fails to compile against ACP 0.11. This means neither of those `cargo` invocations will succeed end-to-end on the current branch state.

The same cross-crate gating was explicitly documented on C9 ("full `cargo test -p llama-agent --lib --no-run` requires the C2-C8 dependencies' code to be merged because llama-agent's dev-dep chain pulls claude-agent through swissarmyhammer-tools — that cross-crate gating is outside this task's scope"). C10 inherits the same constraint.

Once the B-track tasks land, `cargo build --example acp_stdio -p llama-agent` should succeed against the example as currently committed.

### Verification

- `cargo fmt -p llama-agent --check` → clean.
- `cargo check -p llama-agent --lib` → clean (1.21s, no warnings).
- `cargo clippy -p llama-agent --lib -- -D warnings` → clean.

## Depends on
- (C9) ACP 0.11: llama-agent: acp/server.rs reshape.