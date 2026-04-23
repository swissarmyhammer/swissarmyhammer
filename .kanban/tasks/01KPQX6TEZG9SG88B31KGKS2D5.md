---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffa80
project: spatial-nav
title: 'Grid: row selector shows no focus indicator when spatial nav lands on it'
---
## What

User report: "on grids, I cannot navigate to the row selector." The selector IS registered as a spatial target (per 01KPNWH82X) and harness tests prove `h` from the leftmost data cell does move focus to it. The bug is that the production UI gives no visual feedback when the selector is focused — user presses `h`, Rust updates `focused_key`, React's `focusedMoniker` updates, but the selector `<td>` never paints a focus ring, so the user believes nav didn't happen.

Two contributing defects in `kanban-app/ui/src/components/data-table.tsx`:

1. **No `data-focused` / ring on the selector cell.** `RowSelectorTd` (line 881) renders a plain `<td>` without subscribing to focus state. Data cells visually show focus indirectly — `isCursor` in `DataTableCell` applies `ring-2 ring-primary ring-inset` based on `grid.cursor`, which updates when `derivedCursor` resolves the focused moniker via `cellMonikerMap`. But `cellMonikerMap` is built from data columns only (`grid-view.tsx`, `useCellMonikers`). The row selector's moniker uses `ROW_SELECTOR_FIELD = "__rowselector"`, which is intentionally excluded from `cellMonikerMap` so `grid.cursor` can't map to it — and that exclusion leaves the selector with no visual indicator at all.

2. **Click on selector focuses a cell, not the selector.** `DataTableRow` (line 617) wires the selector's onClick to `handleCellClick(dataRowIndex, grid.cursor.col)`, which focuses a data cell at the current cursor column. Keyboard nav focuses the selector; mouse click focuses a cell. Inconsistent and makes manual verification confusing — user clicks the selector to check the focused state and instantly loses it.

### Fix approach

**Primary fix (visual)**: `RowSelectorTd` subscribes to `useFocusedMoniker()` from `@/lib/entity-focus-context`, compares against the `selectorMoniker` prop, and sets `data-focused={isFocused || undefined}` + applies `ring-2 ring-primary ring-inset` (same class used by `DataTableCell`'s `isCursor` branch) when focused. This matches the pattern established by the fixture's `FixtureCellDiv` (`spatial-grid-fixture.tsx:162`). Pure presentational change; no state machine changes.

**Secondary fix (click consistency)**: Keep the grid-cursor side-effect on selector click (the row's data cells need the `isCursorRow` highlight maintained), but ALSO set focus to the selector's moniker so mouse and keyboard converge on the same target. Simplest form: `RowSelector`'s onClick dispatches both `handleCellClick(row, col)` AND `setFocus(selectorMoniker)`. The second call wins — focus lands on the selector — and the grid cursor still advances to the right row.

### Out of scope

- Updating `grid.cursor` to track selector focus directly. That would require changing `derivedCursor` / `cellMonikerMap` to encode a "selector of row N" position (col=-1 or similar), which ripples into edit-mode logic, range selection, and scroll behavior. Not worth it for visual parity — the local `useFocusedMoniker` subscription is simpler and less risky.
- Data cells' focus indicator. They already work correctly via `isCursor`. If anything in this task causes a regression there, revert the data-cell touch; only the row selector needs fixing.

### Files touched

- `kanban-app/ui/src/components/data-table.tsx` — `RowSelectorTd` adds `useFocusedMoniker` subscription + `data-focused` attribute + focus-ring class. `RowSelector` onClick also calls `setFocus(selectorMoniker)`.
- `kanban-app/ui/src/test/spatial-nav-grid.test.tsx` — extend the existing `h from first data cell moves focus to the row selector` test to also assert the selector has `data-focused="true"` and a focus-ring class. Add a new test: clicking the selector sets focus on the selector itself.
- `kanban-app/ui/src/test/spatial-grid-fixture.tsx` — `FixtureCellDiv` also emits `ring-2 ring-primary ring-inset` on the className when focused, so the fixture's selector element mirrors production's new visual contract and the existing spatial-nav-grid test can assert the focus-ring class against the fixture.

## Acceptance Criteria

- [x] After spatial nav lands on the row selector (e.g. `h` from the leftmost data cell), its `<td>` carries `data-focused="true"`
- [x] When focused, the selector `<td>` paints the same focus ring style as a focused data cell (`ring-2 ring-primary ring-inset`)
- [x] Clicking the selector sets `focused_key` to the selector's moniker — verified by the extended spatial-nav-grid test (poll on `data-focused`; equivalent to reading `__spatial_dump().focused_moniker`, which is populated from the same shim state)
- [x] No regression: grid cell nav still passes all tests in `spatial-nav-grid.test.tsx`; `data-table.test.tsx` stays green

Manual smoke checks (pending review):
- [ ] Manual smoke: open a grid, click a leftmost data cell, press `h` — selector is visibly highlighted
- [ ] Manual smoke: click the selector cell — selector is visibly highlighted

## Tests

- [x] Extended `kanban-app/ui/src/test/spatial-nav-grid.test.tsx::"h from first data cell moves focus to the row selector"` — after the existing focus assertion, also asserts `expect(selectorEl.className).toMatch(/ring-2/)` (and the existing `data-focused="true"` poll already runs). RED against HEAD (fixture did not emit ring-2); GREEN after the fixture + production change.
- [x] Added new test `kanban-app/ui/src/test/spatial-nav-grid.test.tsx::"clicking the row selector focuses the selector, not a data cell"` — primes focus on a data cell, then clicks the selector, polls `data-focused="true"` on the selector and `null` on the data cell. The `__spatial_dump` read was substituted by the `data-focused` poll since both read from the same shim state; adding a separate `__spatial_dump` call would duplicate the assertion.
- [x] `cd kanban-app/ui && npm test -- spatial-nav-grid` — 5/5 green (the 4 prior tests + the 1 new click test).
- [x] `cd kanban-app/ui && npm test -- data-table` — 8/8 green (no regression).
- [x] `cd kanban-app/ui && npm test -- focus-scope` — 39/39 green (no regression).

## Workflow

- Use `/tdd` — extend the existing test first (expected to fail at the new assertions), add the click test (expected to fail at its assertion), then add the `useFocusedMoniker` subscription to `RowSelectorTd` and update `RowSelector`'s onClick. Both tests should flip green after minimal edits to `data-table.tsx`.
- Do NOT introduce a new FocusHighlight-style wrapper. Match the fixture's pattern (local subscription in the leaf `<td>`). The row selector's enclosing `FocusScope` stays `renderContainer={false}`; that's correct for a `<td>` consumer.

## Review Findings (2026-04-21 09:17)

Fresh review confirms the overall user contract is met. The implementation mechanism differs from the task description (the visual portion was superseded by 01KPQYE1XMDZ5T538EHSW9TQP5, which centralized `data-focused` writing into `FocusScope.useFocusDecoration` and paints the ring from a global `[data-focused]` CSS rule in `index.css`). The automated acceptance criteria (#1-#4) are verified by tests: `data-focused="true"` appears on the `<td>` after `h`-nav (`spatial-nav-grid.test.tsx:173`), the ring is painted by the global CSS rule (`index.css:148-151`), and clicking the selector focuses the selector's own moniker (`spatial-nav-grid.test.tsx:196-198`). Tests: `spatial-nav-grid` 5/5, `data-table` 8/8, `focus-scope` 43/43 — all green. Code layer checks (design, correctness, naming, tests): clean — `RowSelectorTd` wires its `<td>` via `useFocusScopeElementRef()` with no residual `useFocusedMoniker` subscription or ring className, mirroring `FixtureCellDiv` exactly; `RowSelector.handleClick` composes `onClick` then `setFocus(selectorMoniker)` with the documented ordering (`data-table.tsx:867-870`); the accompanying JSDoc in `RowSelectorTd` accurately describes the new contract.

### Nits
- [ ] `/.kanban/tasks/01KPQX6TEZG9SG88B31KGKS2D5.md` — the two Manual smoke checks remain unchecked. They are human-gated steps (require running the live UI), not code deliverables. Once a human confirms the selector visibly highlights on `h`-nav and on click, checking those two boxes and re-running `/review` will advance the task to `done`.
