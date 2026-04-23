---
assignees:
- claude-code
position_column: todo
position_ordinal: e280
project: spatial-nav
title: Grid ↔ perspective tab bar spatial nav (up/down across the boundary)
---
## SUPERSEDED

This grid-specific task is subsumed by `01KPNWQ844KQBZT59TFJ43TQ31` ("Perspective bar: reachable from every view via top-edge nav"), which covers the grid use case plus board and any future view. Closing this one to avoid duplicated work.

Keep for audit trail:

## What (original)

The perspective tab bar sits above the grid. Today, from a cell in the top row of the grid, pressing `k` cannot reach the perspective tabs — the user is trapped in the grid. The stated purpose of universal spatial nav was "keyboard to anywhere on screen and not get trapped."

Root cause: `ScopedPerspectiveTab` in `kanban-app/ui/src/components/perspective-tab-bar.tsx` uses `CommandScopeProvider` — it registers a scope (so right-click commands resolve) but never registers a spatial rect with Rust. Beam test has no perspective tab candidates.

See `01KPNWQ844KQBZT59TFJ43TQ31` for the live task with TDD plan, vitest-browser harness context, and expanded acceptance criteria.