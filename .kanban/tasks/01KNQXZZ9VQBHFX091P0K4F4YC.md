---
assignees:
- claude-code
depends_on:
- 01KNQXYC4RBQP1N2NQ33P8DPB9
position_column: todo
position_ordinal: a580
project: spatial-nav
title: 'Grid view: wrap as zone, strip legacy keyboard nav'
---
## What

Wrap the grid view in `<FocusZone moniker="ui:grid">`, register each row and cell appropriately, and strip every legacy keyboard-nav vestige from `grid-view.tsx` and `data-table.tsx`. The 2D grid is the cleanest case for spatial nav — cells are laid out in a regular XY grid, so nearest-neighbor by rect is exactly correct.

### Zone shape

```
ui:view (FocusZone, parent)
  ui:grid (FocusZone) ← THIS CARD
    grid_row:{i} (Leaf or Zone, depending on whether rows are themselves navigable units)
      grid_cell:{i,j} (Leaf, one per cell — task field display)
```

**Decision: rows are NOT zones, just visual groupings.** Cells are flat leaves under `ui:grid`. Spatial nav already handles row-and-column movement via beam search. Adding row-zones would force users into drill-in-then-drill-out for cross-row nav, which fights the natural grid UX.

If we later need row-level operations (select-row), we can promote rows to zones. For now: `ui:grid` is the only zone; cells are direct leaf children.

### Files to modify

- `kanban-app/ui/src/components/grid-view.tsx`
- `kanban-app/ui/src/components/data-table.tsx`

### Legacy nav to remove

- `buildCellPredicates()` function (~60 lines) in grid-view.tsx
- `claimPredicates` memo that calls `buildCellPredicates` for every cell
- `cellMonikerMap` (only used by predicates)
- `claimWhen` prop on `<DataTable>` columns and threading through cell rendering
- `ClaimPredicate` import in both files
- Any `onKeyDown` handlers on grid rows / cells / outer container
- Any `keydown` `useEffect` listeners scoped to the grid
- Any roving-tabindex implementation (replaced by spatial-key tracking on each leaf)

What stays: row-virtualization scroll handlers (mouse / scroll wheel), DnD row reorder if present.

### Subtasks
- [ ] Wrap grid container in `<FocusZone moniker={Moniker("ui:grid")}>`
- [ ] Each cell becomes a `<Focusable moniker={Moniker(`grid_cell:${rowIndex}:${colKey}`)}>` leaf with `parent_zone = ui:grid`
- [ ] Delete `buildCellPredicates`
- [ ] Delete `claimPredicates` memo and `cellMonikerMap`
- [ ] Remove `claimWhen` threading through DataTable cell rendering
- [ ] Remove `ClaimPredicate` imports in grid-view.tsx and data-table.tsx
- [ ] Remove all `onKeyDown` / `keydown` handlers from both files
- [ ] Update or remove tests that assert on `buildCellPredicates`

## Acceptance Criteria
- [ ] Grid view registers exactly one Zone (`ui:grid`) at its root, with `parent_zone = ui:view`
- [ ] Every cell registers as a Leaf with `parent_zone = ui:grid`
- [ ] `buildCellPredicates` deleted; ~60 lines removed
- [ ] No `claimWhen` / `ClaimPredicate` / `onKeyDown` / `keydown` references in grid-view.tsx or data-table.tsx
- [ ] Grid cell navigation works identically via spatial nav: up/down/left/right move between cells; `nav.first` / `nav.last` go to top-left / bottom-right; `nav.rowStart` / `nav.rowEnd` operate on the cell's row (via the same-row filter in the algorithm card)
- [ ] `pnpm vitest run` passes

## Tests
- [ ] `grid-view.test.tsx` — grid container is a Zone; cells are leaves with the grid zone as parent_zone
- [ ] `grid-view.test.tsx` — no `claimWhen` props in DataTable column defs
- [ ] `grid-view.test.tsx` — no `keydown` listener attached
- [ ] `grid-view.test.tsx` — existing nav tests pass via spatial nav (regression)
- [ ] `data-table.test.tsx` — cell rendering doesn't accept a `claimWhen` prop
- [ ] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.