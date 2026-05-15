---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffa080
title: Virtualize DataTable rows with @tanstack/react-virtual
---
## What

Virtualize the body rows of `DataTable` (the renderer behind every grid view) using `@tanstack/react-virtual`. Today every data row mounts as a real `<tr>`, so a view with 2k+ entities pays the cost of mounting 2k × N `<td>` + `Field` subtrees on every render — even though only ~20 rows are ever on screen. Virtualizing the rows is the single biggest perf win available for grid views.

**Files:**
- Modify: `kanban-app/ui/src/components/data-table.tsx`
- Modify (probably): `kanban-app/ui/src/components/grid-view.tsx` — only if scroll-into-view for the cursor needs to switch from `cursorRef.scrollIntoView` to `virtualizer.scrollToIndex`. Keep `data-table.tsx`-internal if possible.
- Add test: `kanban-app/ui/src/components/data-table.virtualized.test.tsx`

**Approach:**

1. The dependency is already installed — `@tanstack/react-virtual ^3.13.23` (see `kanban-app/ui/package.json:30`, used by `column-view.tsx::VirtualColumn` for reference).
2. The scroll element is the existing outer `<div className="flex-1 overflow-auto min-h-0">` at `data-table.tsx:95-98`. Attach a `ref` and pass via `getScrollElement`.
3. **Only virtualize body rows.** Leave the existing sticky header (`<TableHeader className="sticky top-0 z-[1] bg-muted/80 backdrop-blur-sm">` at `data-table.tsx:100`) alone — it already pins via CSS sticky and must continue to do so. The user explicitly does NOT want column virtualization; all columns render every row.
4. Use `useVirtualizer` over `flatRows = table.getRowModel().rows` (which includes both data rows and TanStack group-header rows — both need to be virtualized as one flat list, since `dataRowIndices` already maps flat-row index → data-row index).
5. **Fixed row height.** Rows are uniform: every cell uses `px-3 py-1.5` (`data-table.tsx:558`) and renders `<Field mode="compact" />`. Define a `ROW_HEIGHT` constant (start with `32` px — verify by measuring an actual rendered row in the running app and adjust) and use `estimateSize: () => ROW_HEIGHT`. Do NOT use `measureElement` — fixed height makes dynamic measurement unnecessary and avoids the layout-thrash cost. If grouping introduces a different header-row height, `estimateSize: (i) => flatRows[i].getIsGrouped() ? GROUP_ROW_HEIGHT : ROW_HEIGHT` is the escape hatch.
6. **`<table>` virtualization pattern (padding-row technique):** because we render inside a real `<table>`, render two empty `<tr>` spacers — one before the virtualized window with `style={{ height: paddingTop }}` and one after with `style={{ height: paddingBottom }}` — where `paddingTop = virtualItems[0]?.start ?? 0` and `paddingBottom = totalSize - (virtualItems[last]?.end ?? 0)`. Then map `virtualItems.map(vi => <DataTableRow key={flatRows[vi.index].id} row={flatRows[vi.index]} ri={vi.index} ... />)` — keep the existing `DataTableRow` memo signature unchanged.
7. **Cursor scroll-into-view:** the existing `useEffect` at `data-table.tsx:185-187` calls `cursorRef.current?.scrollIntoView()`. With virtualization, the cursor row may not be mounted at all when the user moves the cursor past the visible window. Replace (or augment) with `virtualizer.scrollToIndex(grid.cursor.row)` so the row scrolls into view *first*, then the existing `cursorRef.scrollIntoView` (when the row remounts) handles the inline/horizontal scroll. Be careful: `grid.cursor.row` is a *data-row* index, but the virtualizer indexes into `flatRows`. Use `dataRowIndices.indexOf(grid.cursor.row)` (or build the inverse map) to translate before calling `scrollToIndex`.
8. `overscan: 5` matches `column-view.tsx:844`.
9. Set `count: flatRows.length` so the virtualizer reacts to grouping/expansion changes.

**Out of scope:**
- Column virtualization. Sticky columns stay; do not introduce horizontal virtualization.
- Changing `DataTableRow`/`DataBodyCell`/`GroupHeaderRow` internals — they should be rendered by the virtualizer wrapper without modification.
- Touching `column-view.tsx` (already virtualized).
- Changing `Table` / `TableBody` shadcn primitives (`kanban-app/ui/src/components/ui/table.tsx`).

## Acceptance Criteria

- [x] With a 2000-row grid view rendered, only ~20–30 `<tr data-slot="table-row">` elements exist in the DOM at any one time (initial viewport + overscan), not 2000+.
- [x] Sticky header still pins to the top of the scroll container during scroll (verify visually in dev: `cd kanban-app && bun tauri dev`, scroll a long grid view).
- [x] Cursor navigation (`j`/`k`/arrow keys) past the visible window scrolls the cursor row into view — the cell becomes mounted and visible, focus highlight is correct.
- [x] Grouping still works: collapsing/expanding a group changes the visible row count and the virtualizer adjusts (`count` follows `flatRows.length`).
- [x] Sort header click still works (re-sorts and the virtualized list reflects new order).
- [x] Right-click on whitespace below the last row still fires `onContainerContextMenu` (the bottom padding `<tr>` must not swallow the event — give it `pointer-events: none` if needed, or attach the listener on the outer scroll `<div>` as today).
- [x] No measurable regression on grid views with <50 rows (small grids should not pay overhead beyond a single `useVirtualizer` call).
- [x] No new console warnings/errors during initial mount or scroll.

## Tests

- [x] Add `kanban-app/ui/src/components/data-table.virtualized.test.tsx`. Mock the scroll container's `getBoundingClientRect` (or use `defineProperty` on `clientHeight`) so jsdom reports a finite height — `useVirtualizer` returns 0 items in jsdom otherwise. Reuse the mock setup from `data-table.test.tsx:9-50`.
- [x] Test 1: render `DataTable` with 1000 rows, assert `container.querySelectorAll('tr[data-slot="table-row"]').length` is well under 100 (e.g. `<= 50`). This proves rows are virtualized at all.
- [x] Test 2: cursor scroll-into-view. Render with 1000 rows, programmatically advance `grid.cursor.row` to 500, assert `virtualizer.scrollToIndex` was called (spy on it) OR that the row at index 500 is now in the DOM after a `flushSync` / `await act(...)`. Pick whichever is easier given the existing `useGrid` test fixtures.
- [x] Test 3: existing `data-table.test.tsx` row-structure assertions must still pass unchanged — virtualization should be invisible to those tests, which use small fixtures (<10 rows). Run `cd kanban-app/ui && bun run test data-table` and confirm green.
- [x] Test 4: existing `grid-view.test.tsx` and `grid-view.stale-card-fields.test.tsx` must still pass. Run `cd kanban-app/ui && bun run test grid-view` and confirm green.
- [x] Run full UI suite: `cd kanban-app/ui && bun run test` — no new failures, no new warnings.
- [x] Manual smoke: launch `cd kanban-app && bun tauri dev`, open a grid view with many entities (or seed via the kanban board), scroll to bottom, scroll back to top, navigate with `j`/`k`, group by a column, collapse/expand a group. All should feel snappy with no flicker or missing rows.

## Workflow

- Use `/tdd` — write the virtualization smoke test (test 1 above) first, watch it fail (it currently asserts 1000 rows are rendered), then implement the virtualizer wrapper to make it pass. Then layer in cursor-scroll handling and the remaining tests. #performance #frontend #kanban-app