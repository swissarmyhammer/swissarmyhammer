---
assignees:
- claude-code
depends_on:
- 01KQ4YYFCGJCRN6GBYGVGXVVG6
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffb380
project: spatial-nav
title: 'Board columns: restore scrollability and row virtualization (regression)'
---
## What

Regression: board columns are no longer scrollable, and row virtualization in `column-view.tsx` appears non-functional. Likely introduced during the spatial-nav refactor of `column-view.tsx` (FocusZone wrap, `useStableSpatialKeys`, `usePlaceholderRegistration`, neighbor-moniker prop removal, layout class changes).

## Repro

1. Open the kanban app on a board with a column containing many tasks (more than the virtualization threshold).
2. Observe: column does not scroll vertically — overflow content is clipped or pushes layout.
3. Observe: all task cards render, even off-screen ones (no placeholder rows / virtualizer windowing).

## Root Cause (confirmed)

`<FocusScope kind="zone">` (post-spatial-nav refactor) renders TWO divs: the outer primitive (`<FocusZone>`) which receives `className`, and an inner `FocusScopeBody` chrome wrapper for right-click / double-click / scrollIntoView. That inner `FocusScopeBody` is a plain block `<div>` with no styling — it sits BETWEEN the FocusZone (with `flex flex-col flex-1 min-h-0`) and the column body. A block-display child breaks the flex chain: `flex-1` / `min-h-0` cannot propagate into ColumnBody, so VirtualizedCardList's scroll container loses its constrained height, the virtualizer windows against an unbounded container, and overflow no longer triggers a scrollbar.

Pre-refactor `FocusScope` was a single `FocusHighlight` div, so the chain was preserved.

## Fix

Establish the flex chain BELOW `FocusScopeBody` using absolute positioning (column-view.tsx only — no changes to focus-scope.tsx so the parallel inspector-layout fix is independent):

- Outer `<FocusScope>` is a `position: relative` flex item (`flex-1 min-h-0 ...`) sized by `SortableColumn`'s flex column parent.
- An inner `<div className="absolute inset-0 flex flex-col min-w-0">` escapes `FocusScopeBody`'s static-block layout to re-establish the flex column chain. `inset-0` resolves against the nearest non-static ancestor (the FocusZone div), so the absolute wrapper fills the column's full allocated box regardless of the chrome wrapper between them.
- `ColumnBody` simplified to a `<>` fragment (no redundant inner div) so `ColumnHeader` and `VirtualizedCardList` are direct flex-col children of the absolute wrapper.

## Files

- `kanban-app/ui/src/components/column-view.tsx` (the fix)
- `kanban-app/ui/src/components/column-view.test.tsx` (regression tests)

## Acceptance Criteria

- [x] Each board column scrolls vertically when its task list exceeds visible height
- [x] Row virtualization windowing is active: only ~visible cards mount as primitives; off-screen rows register via `spatial_register_batch` placeholders
- [x] Layout classes preserved: column header stays at top, card list takes remaining height with overflow-y-auto
- [x] No regression in card click / drag / drop / keyboard nav (full suite green: 1567 tests)
- [x] `pnpm vitest run` passes (143 test files, 1567 tests, 0 failures)
- [ ] Manual: open a board with 50+ cards in one column, scroll, confirm visual scrollability and DOM-element count is bounded (deferred to user verification)

## Tests

- [x] `column-view.test.tsx` — assert the scroll container has `overflow-y-auto` class (walks parent chain from drop-zone)
- [x] `column-view.test.tsx` — virtualization threshold test: with N > threshold cards, assert mounted card count < N (uses `stubScrollViewport` pattern from `data-table.virtualized.test.tsx` since the vitest browser project doesn't bundle Tailwind)
- [x] `column-view.test.tsx` — pin the absolute-positioned inner wrapper class structure (`absolute inset-0 flex flex-col`) so a future refactor cannot silently regress
- [x] `column-view.test.tsx` — pin that small lists (below threshold) render all cards directly

## Workflow

- Use `/tdd` — write a failing test for scroll container className first, then for virtualization windowing.

## Origin

User-reported regression on 2026-04-26 during `/finish $spatial-nav` recovery work. Spun out so the spatial-nav scope can finish without blocking on this fix.