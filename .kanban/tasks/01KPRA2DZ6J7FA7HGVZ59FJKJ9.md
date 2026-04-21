---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffd80
project: spatial-nav
title: 'Grid: make column headers spatial nav targets (k from cell lands on header)'
---
## What

In the data-table grid, pressing `k` (up) from a body cell skips right past the column headers and lands on the perspective bar. The column headers are not registered as spatial FocusScopes, so the engine has no intermediate rect to beam-test against — it falls through to the next-higher layer target.

### Target file

`kanban-app/ui/src/components/data-table.tsx`

Relevant components:
- `HeaderCell` (lines 224–277) — renders a `<TableHead>` with a sort/group click handler. **Not** wrapped in a FocusScope today.
- `DataTableHeader` (lines 336–370) — renders `<thead>` → `<tr>` → `HeaderCell[]`. Not wrapped in a FocusScope either.

Body cells already register rects via `DataTableCellTd` using `fieldMoniker(entityType, entityId, fieldName)` → e.g. `field:task:t1.title`. Headers need an analogous rect registration.

### Reference pattern (board columns, already working)

`kanban-app/ui/src/components/column-view.tsx:349-383` uses this shape for its column header:

```tsx
<FocusScope moniker={`${column.moniker}.name`} ...>
  <div className="column-header-focus" onClickCapture={() => setFocus(columnNameMoniker)}>
    ...
  </div>
</FocusScope>
```

Paired CSS in `kanban-app/ui/src/index.css:164-167`:
```css
.column-header-focus[data-focused]::before { left: 0.25rem; }
```

This repositions the focus bar inside the header so it's not clipped.

### Approach

1. **Moniker scheme**: use a new namespace via the existing `moniker(type, id)` helper — `moniker("column-header", fieldName)` produces `column-header:<fieldName>`. This avoids polluting the `field:` namespace with synthetic entity IDs (like `ROW_SELECTOR_FIELD`'s `__rowselector` convention) and mirrors how LeftNav uses `moniker("view", viewId)`.
   - If the codebase ends up supporting multiple grids on screen later, extend to `column-header:<perspectiveId>.<fieldName>`. Out of scope for this task.

2. **Wrap each `HeaderCell` in a `<FocusScope>`** with:
   - `moniker={moniker("column-header", header.column.id)}` (or equivalent — confirm the correct field-name accessor from the `HeaderCell` prop shape)
   - `renderContainer={false}` with `useFocusScopeElementRef()` attached to the `<TableHead>` element — same pattern as `DataTableCellTd` and `RowSelectorTd`. This keeps the table HTML structure valid (no wrapper div inside `<tr>`).
   - `commands={[]}` — no commands specific to the header; keybindings cascade from parent scopes.

3. **Wire setFocus into the click handler** on the `<TableHead>`:
   ```tsx
   onClickCapture={() => setFocus(headerMoniker)}
   ```
   Keep the existing sort/group `onClick` handler — add `setFocus` as a separate `onClickCapture` so focus state updates before the sort toggles.

4. **CSS class for the focus bar**: add `data-table-header-focus` (or reuse `column-header-focus` — they're both header-ish, inside-left positioning is identical). Add a rule to `kanban-app/ui/src/index.css` following the existing `.column-header-focus[data-focused]::before` pattern. Coordinates with `01KPR9Y98HJHBM0NR7P0AWXKEA` (remove-ring task) — whichever lands second will need to merge.

5. **No new commands needed**. The header is just a nav target; Enter on a focused header can be wired separately if we want "sort this column" on the header's Enter — out of scope for this task, file a follow-up if desired.

### Files to modify

- `kanban-app/ui/src/components/data-table.tsx` — wrap `HeaderCell`'s `<TableHead>` in a FocusScope using the ref-forwarding pattern; add `setFocus(headerMoniker)` to an `onClickCapture`.
- `kanban-app/ui/src/index.css` — add a `.data-table-header-focus[data-focused]::before { left: 0.25rem; }` rule (or equivalent).
- `kanban-app/ui/src/lib/moniker.ts` — optional: add a `columnHeaderMoniker(fieldName)` helper for consistency with `fieldMoniker`. If the callsite is only in one place, inline `moniker("column-header", fieldName)` is fine.

## Acceptance Criteria

- [x] Every data-table column header is wrapped in a FocusScope and registers a rect with the Rust spatial engine
- [x] In the grid view, focus on any body cell → press `k` → focus lands on the column header directly above (not on the perspective bar)
- [x] From a focused column header → press `j` → focus returns to a body cell in that column
- [x] Clicking a column header sets spatial focus AND still toggles the existing sort/grouping behavior
- [x] From a focused column header → press `h`/`l` → focus moves to the adjacent column header (left/right)
- [x] Row selector column header (if present) is also reachable by `h` from the leftmost data column header, OR the task explicitly decides to skip the row-selector header and documents why
  - Decision: **skipped**. The row-selector `<TableHead>` is an empty spacer (no label, no sort, no group), so making it a spatial stop would add a keyboard pause with nothing to act on. Documented inline in `DataTableHeader`.

## Tests

- [x] Add a vitest-browser test in `kanban-app/ui/src/components/data-table.test.tsx` (or a new `data-table-header-nav.test.tsx` if the existing file is cluttered) that:
  - Sets up a grid fixture with at least 2 body rows and 3 columns
  - Focuses a body cell at (row 1, col 1)
  - Dispatches the `k` key
  - Asserts the focused moniker changes to `column-header:<colId>` for col 1 (not `perspective:...` or anything upstream)
  - Implemented in `kanban-app/ui/src/test/spatial-nav-grid.test.tsx` — "k from a top-row cell lands on that column's header".
- [x] Add a second test: focus a column header → `j` → focus returns to a body cell in that column ("j from a column header returns focus to a body cell in that column").
- [x] Add a third test: focus a column header → `l` → focus moves to the next column header ("l from a column header moves focus to the next column header").
- [x] Update `kanban-app/ui/src/test/spatial-parity-cases.json` with at least one new case exercising header↔cell vertical nav so both Rust (`swissarmyhammer-spatial-nav/tests/parity.rs`) and JS (`spatial-shim-parity.test.ts`) agree on the outcome — added "grid header row: Up from body cell lands on header above, Right walks header row, Down re-enters body".
- [x] Run `cd kanban-app/ui && npm test` — 1341 tests pass (up from the prior baseline since headers were added to the grid fixture with 3 new grid tests + 2 new data-table.test.tsx tests + 2 new moniker tests).
- [x] Run `cargo test -p swissarmyhammer-spatial-nav` — 55 unit tests + 1 parity test all green with the new case.
- [ ] Manual verification: open the tag grid, click a body cell, press `k` → lands on the column header above. (Not run in agent environment; covered by the automated tests above which exercise the same click + key path against the production `DataTable` + shared fixture.)

## Workflow

- Use `/tdd` — write the three failing nav tests first, then implement to make them pass.
- The moniker scheme here is a judgment call; confirm the chosen convention matches the existing codebase style before coding.

