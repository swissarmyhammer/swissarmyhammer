---
assignees:
- claude-code
depends_on:
- 01KQD0EA8Q367A70AWY8MMZ79D
position_column: todo
position_ordinal: ff9280
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
- [ ] These modules compile under `cargo check -p llama-agent`.
- [ ] One commit on `acp/0.11-rewrite`.

## Tests
- [ ] Inline tests pass.

## Depends on
- 01KQD0EA8Q367A70AWY8MMZ79D (C1).