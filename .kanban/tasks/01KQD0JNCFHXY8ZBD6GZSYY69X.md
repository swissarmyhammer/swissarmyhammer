---
assignees:
- claude-code
depends_on:
- 01KQD0EA8Q367A70AWY8MMZ79D
position_column: todo
position_ordinal: ff9180
project: acp-upgrade
title: 'ACP 0.11: llama-agent: acp/translation.rs'
---
## What

Migrate `llama-agent/src/acp/translation.rs` to ACP 0.11. This is the highest-risk file in llama-agent — extensive `match` blocks over `SessionUpdate`, `ContentBlock`, `ContentChunk`, `ToolKind`, `ToolCallStatus`, `Plan`, `PlanEntry*`, `StopReason`. Every variant addition in 0.11 must be handled explicitly (no `_` catch-alls on previously-exhaustive matches).

## Branch state at task start

C1 (schema imports) landed.

## Acceptance Criteria
- [ ] `acp/translation.rs` compiles under `cargo check -p llama-agent`.
- [ ] One commit on `acp/0.11-rewrite`.

## Tests
- [ ] Inline tests in `translation.rs` pass.

## Depends on
- 01KQD0EA8Q367A70AWY8MMZ79D (C1).