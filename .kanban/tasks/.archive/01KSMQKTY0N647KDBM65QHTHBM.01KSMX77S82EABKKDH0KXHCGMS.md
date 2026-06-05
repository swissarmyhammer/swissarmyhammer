---
assignees:
- claude-code
position_column: todo
position_ordinal: '8280'
title: Switching boards leaves stale perspective view; cards from new board not shown until perspective/view toggled
---
## Bug

When opening or switching to a different board, the previously active perspective filter/view appears to remain applied, so the cards from the newly selected board are not visible until the user manually toggles a perspective or switches view.

## Repro
1. Open board A with a perspective applied that filters cards (e.g. shows only a subset).
2. Switch to board B (or open a different board).
3. Observe: the view still appears scoped by board A's perspective state — board B's cards do not render.
4. Toggle the perspective (or switch view kind) → board B's cards appear correctly.

## Expected
Switching boards should reset/re-resolve the active perspective for the newly opened board so its cards render immediately without manual toggling.

## Likely area
- Perspective state is probably keyed by view kind only (legacy shared-by-kind) or held in a store that isn't invalidated on board switch.
- Check the board-switch / open-board handler — it likely doesn't re-apply or clear the perspective filter against the new board's task set.

#bug #ui #kanban-app