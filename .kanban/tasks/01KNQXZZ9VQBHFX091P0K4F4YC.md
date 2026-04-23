---
assignees:
- claude-code
depends_on:
- 01KNQXYC4RBQP1N2NQ33P8DPB9
position_column: done
position_ordinal: ffffffffffffffffffffffdc80
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
- [x] Delete `buildCellPredicates` function from grid-view.tsx
- [x] Delete `claimPredicates` memo and `cellMonikerMap` from grid-view.tsx
- [x] Remove `claimWhen` threading through DataTable cell rendering
- [x] Verify grid navigation: up/down/left/right/first/last/rowStart/rowEnd all work via spatial nav
- [x] Update or remove tests that assert on `buildCellPredicates`

## Acceptance Criteria
- [x] `buildCellPredicates` deleted — ~60 lines of predicate code removed
- [x] Grid cell navigation works identically via spatial nav
- [x] `nav.rowStart`/`nav.rowEnd` still work (spatial nav handles these via same-row filtering)
- [x] `nav.first`/`nav.last` find top-left / bottom-right cell
- [x] `pnpm vitest run` passes

## Tests
- [x] `kanban-app/ui/src/components/grid-view.test.tsx` — grid navigation tests pass without predicates
- [x] Manual smoke test: grid keyboard navigation feels identical
- [x] Run `cd kanban-app/ui && npx vitest run` — all pass (1111 passed, only pre-existing browser test failure)

## Notes
- `cellMonikerMap` was retained in grid-view.tsx because it is still used by `derivedCursor` for mapping focusedMoniker to grid {row, col}
- `cellMonikers` prop was retained on DataTable for click-to-focus mapping
- `GridCellScope` component and `GridCellScopeProps` interface removed from data-table.tsx
- `FocusScope` import retained in data-table.tsx — still used for row-level entity wrapping