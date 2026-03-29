---
assignees:
- claude-code
depends_on:
- 01KMQMTAMHZHA79PTZAB453KYT
position_column: done
position_ordinal: ffffffffffffff9780
title: Migrate grid navigation to claimWhen
---
## What

Replace the grid's push-based cursor (`useGrid` + `grid.move*` commands + FocusClaim) with pull-based `claimWhen` on each grid cell FocusScope.

### How it works

Each grid cell declares claimWhen predicates based on its row/column neighbors. The grid already renders cells in a known row×col layout, so computing neighbor monikers is straightforward.

### Files to modify

- **`kanban-app/ui/src/components/grid-view.tsx`** — compute claimWhen for each cell, remove `grid.move*` commands, remove FocusClaim
- **`kanban-app/ui/src/hooks/use-grid.ts`** — remove cursor navigation, keep mode (normal/edit/visual) and selection state

### Complexity note

Grid has visual selection mode (`v` key) which selects rectangular regions. This interacts with navigation but is orthogonal to claimWhen — selection expands/contracts based on which cell claims focus. Keep selection logic but decouple it from the cursor.

## Acceptance Criteria

- [ ] j/k/h/l and arrows navigate grid cells via claimWhen
- [ ] Home/End move to row start/end
- [ ] Edit mode still works (i/Enter on focused cell)
- [ ] Visual selection still works with new navigation
- [ ] No grid cursor state — navigation is purely claim-based
- [ ] `pnpm vitest run` passes

## Tests

- [ ] `grid-view.test.tsx` — nav.down from row 0 col 0 focuses row 1 col 0
- [ ] `grid-view.test.tsx` — nav.right from row 0 col 0 focuses row 0 col 1
- [ ] `pnpm vitest run` passes"