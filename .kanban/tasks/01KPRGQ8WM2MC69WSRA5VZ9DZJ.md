---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff80
project: spatial-nav
title: 'Grid: visual focus is driven by grid.cursor in parallel to spatial focus — collapse to one source of truth'
---
## What

On the data-table grid, the first cell (and its row) can appear "focused" even when spatial focus has moved elsewhere. The user sees **two** things highlighted at once. This violates the companion invariant to `01KPRGGCB5NYPW28AJZNM3D0QT`:

> Exactly one scope has the visual focus at any moment — both actually and visually.

### Root cause

The grid maintains `grid.cursor = { row, col, ... }` **independent of spatial focus**, and three separate visual styles are driven off it in `kanban-app/ui/src/components/data-table.tsx`:

| Line | Driver | Visual applied |
|------|--------|----------------|
| 1014 | `isCursorRow && !isEditing` on `EntityRow` (the `<tr>`) | `bg-accent/30` row background |
| 1115 | `isCursorRow` on `RowSelectorTd` | `data-active="true"` attribute |
| 1118 | `isCursorRow` on `RowSelectorTd` | `bg-muted text-foreground` cell background |
| 612  | `isSel && !isCursor` on `DataTableCell` | `bg-primary/10` cell background |

`isCursorRow` comes from `dataRowIndex === grid.cursor.row` at line 773. `isCursor` comes from `dataRowIndex === grid.cursor.row && ci === grid.cursor.col` at line 738. Both derive from `grid.cursor`, not from `useFocusedMoniker()`.

On initial render, `grid.cursor` defaults to `(0, 0)` → the first row gets `bg-accent/30`, the first row selector gets `bg-muted` + `data-active="true"`. If spatial focus lands elsewhere (or hasn't been set yet), the first row still looks focused because nothing about spatial focus has modified `grid.cursor`. That's the bug the user is reporting.

Separately, `FocusScope`'s `useFocusDecoration` already writes `data-focused="true"` on the correctly-focused cell — producing the second, real visual — hence "two things focused at once."

### Approach — `grid.cursor` is no longer an independent source of truth

Spatial focus is THE source of truth for "where the user is." The grid cursor must be a derived view of spatial focus, not a parallel state machine.

1. **Derive `grid.cursor` from spatial focus**. The cursor row/col should be computed from the currently-focused moniker. Parse the cell moniker (`field:<entityType>:<entityId>.<fieldName>`) → map entity id to `dataRowIndex`, field name to column index → that's the cursor. When spatial focus changes, the cursor updates automatically.
   - Primary file: `kanban-app/ui/src/hooks/use-grid.ts` (or wherever `UseGridReturn` is defined — confirm via search)
   - The grid still needs a cursor concept for things like "Enter to edit the current cell," but it derives from `useFocusedMoniker()` instead of maintaining its own row/col state.
   - If spatial focus points to a non-cell moniker (column header, row selector, perspective tab), the cursor is null — no row/column is treated as cursor.

2. **Remove the redundant visuals driven by `isCursorRow`**. Spatial focus's `data-focused` already paints the target. Delete:
   - Line 1014: `isCursorRow && !isEditing && "bg-accent/30"` — the `<tr>` background
   - Line 1115–1118: `data-active` attribute and `bg-muted text-foreground` className on the row selector
   - Line 612: `isSel && !isCursor && "bg-primary/10"` — actually this one is for selection state (multi-select), not cursor state. Keep `isSel`-only; drop the `!isCursor` qualifier since there's no longer a cursor-driven visual competing with it. Result: `isSel && "bg-primary/10"`.

3. **Keep `isCursor` for `cursorRef` binding** (line 625). That's infrastructure (scroll-into-view target), not a visual. `isCursor` becomes derived from spatial focus too: `focusedMoniker === cellMoniker`.

4. **No new `data-focused` logic**. `FocusScope` via `useFocusDecoration` is the only code that writes `data-focused`. Existing `.cell-focus[data-focused]::before` CSS handles the visual.

### Relationship to other tasks

- **`01KPRGGCB5NYPW28AJZNM3D0QT`** (always-something-focused invariant) — companion. That task enforces `≥ 1` focused scope; this task enforces `≤ 1` visually focused element. Together: exactly one.
- **`01KPR9Y98HJHBM0NR7P0AWXKEA`** (remove ring, only left bar) — independent but touches adjacent CSS. If this task lands first, the row's `bg-accent/30` is gone so the ring-or-bar question is simpler. No strict `depends_on`.
- **`01KPRA2DZ6J7FA7HGVZ59FJKJ9`** (column headers as nav targets) — independent. Column headers don't use `isCursorRow`.

### Files to modify

- `kanban-app/ui/src/components/data-table.tsx` — derive `isCursor`/`isCursorRow` from spatial focus (or replace with direct `focusedMoniker` comparison); remove the `bg-accent/30`, `bg-muted`, `data-active` visuals
- `kanban-app/ui/src/hooks/use-grid.ts` — rewire `grid.cursor` to derive from `useFocusedMoniker()` rather than maintain independent state. Keep the `grid.mode` / `enterEdit` / `exitEdit` surface intact.
- Test fixtures that assume `grid.cursor` is independently settable — update to set spatial focus instead.

## Acceptance Criteria

- [ ] On initial grid render with no click yet, no cell or row shows any focus-like visual until spatial focus claims one (or the always-focused invariant from `01KPRGGCB5NYPW28AJZNM3D0QT` picks a first cell — in which case **only** that cell shows the visual)
- [ ] At any moment during navigation, **exactly one** DOM element in the grid has `data-focused="true"`, and no other element has a cursor-driven background (`bg-accent/30`, `bg-muted`, `bg-primary/10` on non-selected cells)
- [ ] No `data-active="true"` attribute appears on `RowSelectorTd` (the attribute was redundant with `data-focused`)
- [ ] Clicking a cell moves spatial focus to that cell AND the grid's internal cursor follows (so keyboard ops like Enter-to-edit work on the clicked cell)
- [ ] Pressing `h/j/k/l` moves spatial focus AND the grid cursor follows — no cell is ever left visually highlighted after focus moves away
- [ ] `cursorRef` (scroll-into-view) still points at the focused cell so it scrolls into view

## Tests

- [ ] Add a vitest-browser test in `kanban-app/ui/src/components/data-table.test.tsx`: render a grid, focus cell (1, 1), assert only one `[data-focused]` element in the DOM, assert no `.bg-accent/30` row, no `.bg-muted` selector — then `l` to (1, 2), re-assert the invariants
- [ ] Add a test: initial render with no click, assert no element has `data-focused` OR exactly one does (whichever the always-focused invariant lands on) — but never two
- [ ] Add a test: click cell (2, 1) → assert the grid's internal cursor state reports `row=2, col=1` (i.e. cursor follows spatial focus)
- [ ] Add a parity case in `kanban-app/ui/src/test/spatial-parity-cases.json` exercising cursor-follows-focus — Rust side doesn't need changes since cursor is a pure UI concept, but the JS shim test should verify it
- [ ] Run `cd kanban-app/ui && npm test` — all 1301+ tests still pass
- [ ] Manual: load the tag grid, do nothing → at most one visual focus on screen. Click cell (3, 2) → only cell (3, 2) visible as focused. Press `j` → only (4, 2) visible as focused. Verify the first cell never retains its background.

## Workflow

- Use `/tdd` — write the "exactly one visual focus" assertions first against the current code (they will fail), then implement.
- If `grid.cursor` has callers outside `data-table.tsx` (e.g. column sort, row selection, command bindings), audit them — each caller needs to read from the new derived source.

