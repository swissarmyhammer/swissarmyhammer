---
assignees:
- claude-code
depends_on:
- 01KQD0EA8Q367A70AWY8MMZ79D
position_column: todo
position_ordinal: ff9480
project: acp-upgrade
title: 'ACP 0.11: llama-agent: filesystem + terminal handlers'
---
## What

Migrate filesystem/terminal handler modules to ACP 0.11.

Files:
- `llama-agent/src/acp/filesystem.rs`
- `llama-agent/src/acp/terminal.rs`

## Branch state at task start

C1 landed.

## Acceptance Criteria
- [ ] These modules compile under `cargo check -p llama-agent`.
- [ ] One commit on `acp/0.11-rewrite`.

## Tests
- [ ] Inline tests pass.

## Depends on
- 01KQD0EA8Q367A70AWY8MMZ79D (C1).