---
assignees:
- claude-code
depends_on:
- 01KQD0NNG9DPHWATDNN61EERE2
position_column: todo
position_ordinal: ff9d80
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
- [ ] `cargo check -p llama-agent --lib --examples` passes for these targets.
- [ ] `cargo build --example acp_stdio -p llama-agent` succeeds.
- [ ] One commit on `acp/0.11-rewrite`.

## Tests
- [ ] Inline tests in `agent.rs` pass.

## Depends on
- (C9) ACP 0.11: llama-agent: acp/server.rs reshape.