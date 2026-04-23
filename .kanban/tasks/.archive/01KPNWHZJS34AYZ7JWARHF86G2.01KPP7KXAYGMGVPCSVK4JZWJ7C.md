---
assignees:
- claude-code
position_column: todo
position_ordinal: e380
project: spatial-nav
title: Grid ↔ LeftNav (view switcher) spatial nav (left boundary)
---
## SUPERSEDED

This grid-specific task is subsumed by `01KPNWPX9NWSVGTJAHB4Z1VSED` ("Nav bar (LeftNav): reachable from every view via left-edge nav"), which covers the grid use case plus board and any future view. Closing this one to avoid duplicated work.

Keep for audit trail:

## What (original)

The LeftNav strip on the left edge of the window contains the view-switcher buttons. Today, from a row-selector cell in the grid, pressing `h` cannot reach the LeftNav — the user is trapped. The stated purpose of universal spatial nav was "keyboard to anywhere on screen and not get trapped."

Root cause: `LeftNav` in `kanban-app/ui/src/components/left-nav.tsx` renders plain `<button>` elements with `onClick` dispatches. No FocusScope, no spatial entry.

See `01KPNWPX9NWSVGTJAHB4Z1VSED` for the live task with TDD plan, vitest-browser harness context, and expanded acceptance criteria.