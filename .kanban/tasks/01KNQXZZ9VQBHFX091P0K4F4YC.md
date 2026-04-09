---
assignees:
- claude-code
depends_on:
- 01KNQXYC4RBQP1N2NQ33P8DPB9
position_column: todo
position_ordinal: a580
project: spatial-nav
title: Remove manual claimWhen predicates from grid-view
---
## What

Delete `buildCellPredicates` and all manual predicate construction from grid-view. The 2D grid is the ideal case for spatial nav — cells are laid out in a regular XY grid, so nearest-neighbor by rect is exactly correct.

### Files to modify

1. **`kanban-app/ui/src/components/grid-view.tsx`**:
   - Delete `buildCellPredicates()` function (~60 lines)
   - Delete `claimPredicates` memo that calls `buildCellPredicates` for every cell
   - Delete `cellMonikerMap` (only used by predicates)
   - Remove `claimWhen` prop from `<DataTable>` columns
   - Remove `ClaimPredicate` import

2. **`kanban-app/ui/src/components/data-table.tsx`**:
   - Remove `claimWhen` prop from cell rendering if it's threaded through here
   - Remove `ClaimPredicate` import if present

### Subtasks
- [ ] Delete `buildCellPredicates` function from grid-view.tsx
- [ ] Delete `claimPredicates` memo and `cellMonikerMap` from grid-view.tsx
- [ ] Remove `claimWhen` threading through DataTable cell rendering
- [ ] Verify grid navigation: up/down/left/right/first/last/rowStart/rowEnd all work via spatial nav
- [ ] Update or remove tests that assert on `buildCellPredicates`

## Acceptance Criteria
- [ ] `buildCellPredicates` deleted — ~60 lines of predicate code removed
- [ ] Grid cell navigation works identically via spatial nav
- [ ] `nav.rowStart`/`nav.rowEnd` still work (spatial nav handles these via same-row filtering)
- [ ] `nav.first`/`nav.last` find top-left / bottom-right cell
- [ ] `pnpm vitest run` passes

## Tests
- [ ] `kanban-app/ui/src/components/grid-view.test.tsx` — grid navigation tests pass without predicates
- [ ] Manual smoke test: grid keyboard navigation feels identical
- [ ] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.