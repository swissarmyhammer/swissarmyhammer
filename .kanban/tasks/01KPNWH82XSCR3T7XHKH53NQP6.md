---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffff380
project: spatial-nav
title: 'Grid: cell-to-cell spatial nav (h/j/k/l) with row selector column as a nav target'
---
## What

In the grid view (e.g. tags grid, tasks grid, any perspective using `DataTable`):

- `h` / `l` moves horizontally between cells in the same row
- `j` / `k` moves vertically between cells in the same column
- The leftmost column (row selector with the ordinal number) is ALSO a nav target — `h` from the first data cell moves into the row selector; `l` from the row selector moves to the first data cell

Today, none of this works reliably. At the end of this session grid nav was "jumping all over again" after the row was marked non-spatial and various FocusScopes were added without tests.

### Root-cause hypotheses (unverified — confirm via tests)

- Rows (`<tr>`) register a spatial entry covering the whole row; cells register their own entries inside. When navigating from a cell, the parent row's rect shows up as a beam-test candidate and gets picked ahead of the intended sibling cell.
- Cells don't have FocusScopes today, so `setFocus(cellMoniker)` sets a focused_moniker for which `getScope()` returns null, the scope chain walk is empty, and grid nav commands aren't found during keybinding resolution.
- Row selectors are plain `<td>` elements, not FocusScopes — no spatial entry, no moniker, unreachable.

### TDD — failing tests (write these FIRST)

Under `kanban-app/ui/src/test/spatial-nav-grid.test.tsx` (using the vitest-browser harness):

```ts
describe("grid cell-to-cell spatial navigation", () => {
  it("h/j/k/l moves between field cells in the active grid", async () => {
    // Click cell (row 0, col "tag_name"), press j → focus lands on (row 1, col "tag_name")
  });

  it("h from first data cell moves focus to the row selector", async () => {
    // Click cell (row 3, col tag_name, the leftmost data column).
    // Press h → focus lands on the row-3 selector.
  });

  it("l from the row selector moves focus to the first data cell in the same row", async () => {
    // Click row-3 selector. Press l → focus lands on (row 3, col 0).
  });

  it("j from the last row of cells stays put (does not wrap)", async () => {
    // Click last row. Press j. Focus does not change.
  });
});
```

All four tests must fail against HEAD before any implementation change.

### Likely fix outline (confirm only against failing tests)

1. Wrap each `DataTableCell`'s content in a `FocusScope` with the cell's field moniker; attach the cell's `FocusScope` element ref to the `<td>` so ResizeObserver can measure it.
2. Wrap the `RowSelector` cell in a `FocusScope` with a reserved moniker (e.g. `fieldMoniker(entityType, entityId, "__rowselector")`); same ref-attachment pattern.
3. Make the row `<tr>` FocusScope non-spatial (scope registration only, no rect) so it doesn't shadow cell candidates during beam tests. Requires a `spatial?: boolean` prop on `FocusScope` (default true); only rows opt out.
4. The "attach the ref to the consumer's DOM element when `renderContainer={false}`" pattern needs a small context (`FocusScopeElementRefContext`) so `<td>` / `<tr>` consumers can grab the ref.

All four of those changes were attempted this session without tests; every combination we tried broke something else. This time they land one failing-test-at-a-time.

### Subtasks

- [x] Write failing E2E: `h/j/k/l` between field cells
- [x] Write failing E2E: `h` → row selector
- [x] Write failing E2E: `l` from row selector → first data cell
- [x] Write failing E2E: `j` at last row clamps (no wrap)
- [x] Implement only what's required to make each test pass (TDD)
- [x] Do not ship any FocusScope prop or context change that isn't exercised by a test

### Acceptance

- [x] All four failing tests now pass
- [x] No regression in board / inspector nav (existing tests green — 1293 tests across 121 files)
- [x] Grid behaves consistently — no "jumps to first cell and sticks" intermittent (tracked separately)

## Review Findings (2026-04-20 14:40)

### Nits
- [x] `kanban-app/ui/src/components/data-table.tsx:53` and `kanban-app/ui/src/test/spatial-grid-fixture.tsx:88` — The reserved field name `"__rowselector"` is duplicated in production and test fixture as `ROW_SELECTOR_FIELD`. Both files carry cross-referencing comments that say "must stay in sync", which is a known code smell. Extract a single exported constant (e.g. in `@/lib/moniker` or a new `@/lib/row-selector`) and import from both sites so the two cannot drift.
  - Fixed by exporting `ROW_SELECTOR_FIELD` from `@/lib/moniker` and importing it in both `components/data-table.tsx` and `test/spatial-grid-fixture.tsx`. The sync comments are gone; the constant lives next to `fieldMoniker` where row-selector monikers are built.
- [x] `kanban-app/ui/src/components/focus-scope.test.tsx` — The new `spatial?: boolean` prop on `FocusScope` and the new `useFocusScopeElementRef` hook have no direct unit tests in this file. Their behavior is exercised end-to-end by `spatial-nav-grid.test.tsx`, which is adequate coverage for the feature, but a targeted assertion (`spatial={false}` does not invoke `spatial_register`; `useFocusScopeElementRef()` returns a non-null ref only under `renderContainer={false}`) would localize the API contract and make future regressions point-of-failure clearer. The task explicitly says "Do not ship any FocusScope prop or context change that isn't exercised by a test" — integration coverage satisfies that, but a direct test would harden the surface.
  - Fixed by adding two new describe blocks to `components/focus-scope.test.tsx`: `FocusScope spatial prop` (4 tests — `spatial=false` skips register, skips unregister on unmount, default still registers, scope stays in the entity-focus registry) and `useFocusScopeElementRef` (4 tests — null outside any scope, null under `renderContainer=true`, non-null under `renderContainer=false`, and a full round-trip test proving the returned ref is the one the scope observes for rect reporting). Total focus-scope.test.tsx count: 38 tests (up from 30).

## Resolution (2026-04-20 — review nits pass)

Both review nits resolved. Full ui test suite: `npm run test` → **1301 passed** across 121 files (+8 from previous 1293). `tsc --noEmit` clean.