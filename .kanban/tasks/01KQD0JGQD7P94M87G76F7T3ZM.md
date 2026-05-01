---
assignees:
- claude-code
depends_on:
- 01KQD0EA8Q367A70AWY8MMZ79D
position_column: done
position_ordinal: ffffffffffffffffffffffffa180
project: acp-upgrade
title: 'ACP 0.11: llama-agent: session types (types/sessions + acp/session)'
---
## What

Migrate session-state modules to ACP 0.11.

Files:
- `llama-agent/src/types/sessions.rs`
- `llama-agent/src/acp/session.rs`

## Branch state at task start

C1 (schema imports) landed.

## Acceptance Criteria
- [ ] These modules compile under `cargo check -p llama-agent`.
- [ ] One commit on `acp/0.11-rewrite`.

## Tests
- [ ] Inline tests in these files pass.

## Depends on
- 01KQD0EA8Q367A70AWY8MMZ79D (C1).