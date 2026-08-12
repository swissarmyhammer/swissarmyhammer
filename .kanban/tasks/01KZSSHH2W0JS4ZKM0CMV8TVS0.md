---
assignees:
- claude-code
position_column: todo
position_ordinal: ffce80
title: Share one djb2 actor-colour helper instead of three copies
---
## Problem

The same djb2 hash-to-palette helper is written three times, each with its own
palette:

- `crates/swissarmyhammer-tools/src/mcp/server/agent_identity.rs::agent_deterministic_color`
- `apps/kanban-app/src/state.rs::deterministic_color`
- `apps/kanban-app/ui/src/lib/actor-colors.ts::deriveActorColor`

`code_context find duplicates` measures the first two at 96.9% alike and the
third at 87.5%. A fourth, `crates/swissarmyhammer-kanban/src/auto_color.rs`,
does the same job with FNV-1a and a fourth palette.

## Task

Put one hash in `swissarmyhammer-kanban` — the crate both the tools crate and
the kanban app already depend on — and call it from each Rust site. Keep each
palette where it is; only the hash is shared. Decide whether the TypeScript
copy must stay a copy, and say why in the code if it must.

## Found by

Card ^hxd1r4r, while naming the djb2 constants in `agent_identity.rs`. Out of
that card's scope because the fix crosses three crates.