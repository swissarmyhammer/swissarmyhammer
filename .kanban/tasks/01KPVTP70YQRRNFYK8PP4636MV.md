---
assignees:
- claude-code
position_column: todo
position_ordinal: e280
project: spatial-nav
title: 'Data-table grid: virtualize rows, and keep spatial nav correct through the virtualized layer'
---
## What

The data-table grid in `kanban-app/ui/src/components/data-table.tsx` currently renders every row in the DOM — no virtualization. This is functionally fine at small row counts and is incidentally why spatial nav works reliably in the grid (every row's rect is accurate because natural DOM flow + `overflow: scroll` container doesn't break `getBoundingClientRect()` semantics the way transform-based virtualization does in `column-view.tsx`). It is **not** fine at scale: a board with thousands of tasks in a grid view will mount thousands of `<tr>` + N × `<td>` + N × `<FocusScope>` structures, re-run every effect on every filter change, and thrash the scope registry.

Two concerns to address together:

1. **Add virtualization to the data-table grid** using the same `@tanstack/react-virtual` the board columns use.
2. **Make sure spatial nav stays correct after virtualization** — the fix from task `01KPVTKZ1VGDSBB0HPYTTAHJNH` (report rects on scroll for virtualized FocusScopes) MUST be the mechanism that keeps nav accurate here. No grid-specific nav workaround.

### Why this is one task

The changes are tightly coupled. Virtualizing the grid without the scroll-rect-reporting fix would break nav the same way board cards are broken today. Shipping them separately risks either (a) a known-broken intermediate state where grid nav regresses after virtualization, or (b) re-testing the entire grid nav surface twice.

### Dependencies and ordering

- **Depends on** `01KPVTKZ1VGDSBB0HPYTTAHJNH` landing first. That task establishes the general mechanism (scroll-listener + RAF-throttled rect re-report in `useRectObserver`). This task's job is to apply virtualization, confirm the general mechanism carries over, and add grid-specific tests — not to reinvent rect reporting.
- **Companion of** `01KPVT95H4FTCC5Q4E7G644CHD` (perspective tab nav) and `01KPVT4K538CJHJR31NNQHY8EH` (inspector layer escape). Those are separate bugs with separate fixes; don't fold them in here.

### Approach

1. **Mirror the column-view virtualizer pattern.** `column-view.tsx:672-713` is the working reference — `useVirtualizer` with `getScrollElement`, `estimateSize`, `overscan`, absolute-positioned children, height reservation via `getTotalSize()`. Data-table's natural structural unit is the row (not cells), so virtualize at the row level. Each virtual row renders one `<tr>` positioned absolutely with `transform: translateY(start)`, containing its cells as they are today.

2. **Row-level FocusScopes need scroll-linked rect updates.** After task `01KPVTKZ1VGDSBB0HPYTTAHJNH` lands, `useRectObserver` will listen to the nearest scrollable ancestor's `scroll` event. Confirm the `<table>`'s scroll container is recognized as that ancestor — if `overflow: scroll` is on an outer wrapper rather than the `<table>` itself, the walk-up logic must still find it.

3. **Cell-level FocusScopes** (per `DataTableCellTd`, `RowSelectorTd`, column headers) inherit the scroll-listener from the same mechanism — they're descendants of the same scroll container. Verify the ResizeObserver + scroll-listener combination covers both row-position changes (scroll) and column-width changes (resize).

4. **Column headers** must stay fixed while the body scrolls. Standard pattern: `<thead>` with `position: sticky; top: 0`. Sticky elements keep `getBoundingClientRect()` accurate without transforms — so headers' rects update naturally. Confirm this works with the existing `HeaderCell` FocusScope wiring.

5. **Handle the "nav past visible edge" case.** Same deferred decision as task `01KPVTKZ1VGDSBB0HPYTTAHJNH`: pressing `j` at the bottom of the visible viewport, when more rows exist below that are unmounted, should either (a) auto-scroll the container to reveal the next row and land focus there, or (b) stop at the current last visible row. Default to (b) for this task — auto-scroll-on-edge is a follow-up.

### Files to modify

- `kanban-app/ui/src/components/data-table.tsx` — introduce `useVirtualizer`, refactor `<tbody>` rendering to map virtual rows, add scroll container wrapping the table
- `kanban-app/ui/src/hooks/use-grid.ts` — if the grid cursor state model assumes all rows are mounted (e.g. to find a row by dataRowIndex), audit for virtual-row compatibility. The cursor derivation from focused moniker (per `01KPRGQ8WM2MC69WSRA5VZ9DZJ`) should be unaffected because monikers are entity-scoped, not index-scoped.
- `kanban-app/ui/src/components/data-table.test.tsx` — update existing tests that render "every row visible" assumptions; add virtualized-specific tests

### Files NOT to modify

- `kanban-app/ui/src/components/focus-scope.tsx` — the scroll-listener-in-useRectObserver change lives in `01KPVTKZ1VGDSBB0HPYTTAHJNH`. Do NOT duplicate it here.
- `swissarmyhammer-spatial-nav/src/` — Rust unchanged.

### Performance target

- Rendering a grid with 10,000 tasks must not mount 10,000 `<tr>`. Confirm via browser devtools that rendered `<tr>` count after virtualization is roughly `visible_viewport_rows + 2 * overscan` (the same pattern column-view uses).
- `spatial_register` invoke count after initial mount should match visible-row count, not total-row count.

## Acceptance Criteria

- [ ] `data-table.tsx` uses `useVirtualizer` with a sensible `overscan` (start with the same constant as `column-view.tsx` — `VIRTUALIZER_OVERSCAN`)
- [ ] With 10,000 rows in the grid, rendered `<tr>` count is < 100 (visible viewport + overscan)
- [ ] Spatial nav inside the grid still works: pressing `j`/`k` moves focus row by row; `h`/`l` moves across cells; the row selector column still focuses correctly
- [ ] After scrolling the grid, pressing `j` from a visible row lands on the next visible row — not a stale-coordinate target
- [ ] Column headers remain visible (sticky) while the body scrolls; header-cell FocusScopes register rects at their current sticky position, nav from header cells works
- [ ] `__spatial_dump` after a scroll shows each mounted row's `rect.y` matches its actual on-screen position (to within 1px)
- [ ] Pressing `j` at the bottom visible row when more rows exist below does NOT crash or escape to another layer — it either stays put (preferred default) or auto-scrolls (if that's already implemented by a sibling task)
- [ ] No regression in existing grid tests — cursor behavior, sort, row-selection, edit-mode, inspector-open-from-row-selector
- [ ] All existing tests green (`cd kanban-app/ui && npm test` and `cargo test -p swissarmyhammer-spatial-nav -p swissarmyhammer-kanban -p kanban-app`)

## Tests

- [ ] Add a vitest-browser test in `kanban-app/ui/src/components/data-table.test.tsx` that mounts a grid with >100 rows and asserts the rendered `<tr>` count is bounded to ~viewport+overscan, not the full row count
- [ ] Add a nav-under-scroll test: mount a grid with >50 rows, scroll down so row 25 is at the top of viewport, press `j` from row 25, assert focus lands on row 26 (not a stale rect's target)
- [ ] Add a header-stays-fixed test: scroll the grid body, assert column header FocusScopes still have accurate rects and nav from a header cell to the first body cell still works
- [ ] Run `cd kanban-app/ui && npm test` — green
- [ ] Manual: load a real board with the largest grid you have, scroll around, nav with `h/j/k/l` — feels responsive, focus bar consistently lands where expected

## Workflow

- Use `/tdd`. Write the "10k rows → <100 rendered" test and a failing "scrolled nav lands correctly" test first.
- Land `01KPVTKZ1VGDSBB0HPYTTAHJNH` before starting this — the scroll-listener mechanism must already exist.
- If the scroll-listener from `01KPVTKZ1VGDSBB0HPYTTAHJNH` doesn't find the grid's scroll container correctly (e.g. `overflow: scroll` is on a different ancestor than it is in column-view), fix it in that task's code, not here. The mechanism must generalize; if it doesn't, the earlier task is underbaked.
- Do not expand scope to auto-scroll-on-edge. File a separate task.
- Do not change the grid cursor model beyond what virtualization requires — the cursor stays a derived view of focused moniker per `01KPRGQ8WM2MC69WSRA5VZ9DZJ`.

