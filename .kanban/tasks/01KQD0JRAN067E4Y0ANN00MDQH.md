---
assignees:
- claude-code
depends_on:
- 01KQD0EA8Q367A70AWY8MMZ79D
position_column: done
position_ordinal: ffffffffffffffffffffffff9180
project: acp-upgrade
title: 'ACP 0.11: llama-agent: error + config + mod (acp glue)'
---
## What

Migrate the small acp glue modules to ACP 0.11.

Files:
- `llama-agent/src/acp/error.rs`
- `llama-agent/src/acp/config.rs`
- `llama-agent/src/acp/mod.rs`

## Branch state at task start

C1 landed.

## Acceptance Criteria
- [x] These modules compile under `cargo check -p llama-agent`.
- [x] One commit on `acp/0.11-rewrite`.

## Tests
- [x] Inline tests pass.

## Notes

C1 (commit 9f4f564d0) already migrated all schema-type references in
`config.rs` to `agent_client_protocol::schema::*`. `error.rs` has no
ACP type references at all (pure thiserror types). `mod.rs` only
re-exports local items and contains rustdoc comments — no migration
needed.

Verification on `acp/0.11-rewrite`:
- `cargo check -p llama-agent` produces zero errors and zero warnings
  attributable to these three files. The remaining lib errors all
  originate in `acp/server.rs` (impl Agent for AcpServer,
  AgentWithFixture import) and are tracked by separate tasks.
- All inline test code in `error.rs` and `config.rs` references only
  local types and standard library; no ACP-0.10/0.11 API surface
  changes affect them.

## Depends on
- 01KQD0EA8Q367A70AWY8MMZ79D (C1).