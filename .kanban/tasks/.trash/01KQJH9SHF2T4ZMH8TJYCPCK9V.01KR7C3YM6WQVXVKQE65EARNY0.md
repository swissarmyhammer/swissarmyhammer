---
assignees:
- wballard
position_column: todo
position_ordinal: b180
title: Grid cell hit area = full cell via Field padding prop (drop redundant GridCellFocusable scope)
---
## What

In the grid view, keyboard focus (and the focus indicator) only covers the inner content of each cell, not the visible cell box. The fix is **not** to add yet another zone wrapping the cell — that would compound the existing redundant-zone problem. Instead, give `Field` the ability to own its own padding so its existing registered `FocusZone` border-box becomes the full cell box.

### Today's structure (the problem)

`kanban-app/ui/src/components/data-table.tsx:931-943` (`GridCellFocusable`):

```tsx
<TableCell ref={cursorRef} className={className} data-cell-cursor={cellCursorAttr}>
  <FocusScope moniker={moniker}>
    <div onClick={onClick} onDoubleClick={onDoubleClick}>
      {children /* <Field mode="compact" handleEvents={false} /> */}
    </div>
  </FocusScope>
</TableCell>
```

And cell padding lives on the outer `<TableCell>`:

```tsx
const cellClasses = cn("px-3 py-1.5 align-middle max-w-[300px]", ci === 0 && "pl-4", …);
```

Two problems:

1. **Padding outside the registered rect.** The kernel reads `getBoundingClientRect()` on the FocusScope host (`focus-scope.tsx:322-340`) — which is the inner unpadded `<div>`. Padding sits outside, so the focus rect is smaller than the visible cell.
2. **Redundant zone.** `<Field>` already renders its own `<FocusZone>` (`field.tsx:621-628`). `GridCellFocusable` wraps it in *another* `<FocusScope>` — two registrations per cell, exactly the duplication the user wants eliminated.

### Fix direction

1. **Add an optional `padding` (or `containerClassName` — see decision note) prop to `Field`** in `kanban-app/ui/src/components/fields/field.tsx`. Forward it to the className on Field's `<FocusZone>` host element. Default empty → behavior unchanged for every existing caller.
2. **Remove `GridCellFocusable`'s `<FocusScope>` wrapping in `data-table.tsx`** (or fold it down so the cell registers a single zone — Field's own). Pass the `px-3 py-1.5` (and `pl-4` for first column) padding via the new Field prop instead of via `<TableCell className>`.
3. The cell's outer `<TableCell>` becomes a bare layout container (no padding). Click/dblclick handlers and `data-cell-cursor` move to whatever element survives the consolidation — preserve the existing onClick/onDoubleClick semantics.

Result: one zone per cell (Field's own), border-box equals the full visible cell box, focus rect covers padding. No redundant nesting.

### Decision note (for implementer)

Decide between:

- (a) `padding?: string` — typed narrowly to a Tailwind padding class, communicates intent.
- (b) `containerClassName?: string` — generic forwarding seam onto the FocusZone host.

(a) is more opinionated and matches the user's request literally; (b) is more general but invites misuse. **Recommend (a)** unless the implementer finds another grid-like consumer that needs non-padding layout classes — keep the seam narrow.

### Non-goals

- Do **not** add another outer `FocusScope` or `FocusZone` around the cell. The whole point of this task is to *eliminate* a redundant zone, not add one.
- Do **not** change Field's behavior for non-grid callers (Inspector rows, `<EntityCard>` cells, nav-bar, board-selector, column-view title). Default empty padding preserves their layout.
- Do **not** introduce a `padding` prop that bypasses `<FocusZone>` (e.g. wraps it in another padded div) — the padding MUST land on the FocusZone's host element so `getBoundingClientRect` includes it.

### Files to modify

- `kanban-app/ui/src/components/fields/field.tsx` — add the `padding` prop to the `FieldProps` interface (~line 213-276), forward it to `<FocusZone className=…>` (~line 621-628). FocusZone already accepts className passthrough (`focus-zone.tsx:141-142, 499-516`).
- `kanban-app/ui/src/components/data-table.tsx` — collapse `GridCellFocusable` so each cell renders a single registered zone via `<Field padding={cellPaddingClasses}>`. Move `px-3 py-1.5` (and `pl-4` for column 0) off `<TableCell>` and into the `padding` prop. Preserve `cellCursorAttr`, click, and double-click semantics.
- (Possibly) `kanban-app/ui/src/components/grid-view.tsx` — if cell wiring also lives there, mirror the change.

## Acceptance Criteria

- [ ] Pressing Tab/arrow into a grid cell: the focus indicator rect equals the **full visible cell box** (including the padding currently on `<TableCell>`), not just the inner content.
- [ ] Clicking anywhere within a cell's padding focuses that cell (bigger hit area).
- [ ] Each grid cell registers exactly **one** spatial scope (Field's own `<FocusZone>`) — verified by counting `spatial_register_scope`/`spatial_register_zone` calls per cell render.
- [ ] No regression for non-grid Field consumers: Inspector rows, `<EntityCard>` cells, nav-bar, etc. visually and behaviorally unchanged (default `padding` empty/undefined).
- [ ] Existing grid keyboard-nav tests (`grid-view.spatial-nav.test.tsx`, `grid-view.nav-is-eventdriven.test.tsx`) still pass.

## Tests

- [ ] **TDD: write the failing unit test first** in `kanban-app/ui/src/components/fields/field.spatial-nav.test.tsx`. Render `<Field padding="px-3 py-1.5" …>` and assert the captured `spatial_register_zone` rect width/height includes the padding (compare to the same Field without `padding`). Run, confirm it fails, then implement the prop.
- [ ] Add a regression test in `kanban-app/ui/src/components/data-table.test.tsx` (or a new `grid-view.cell-hit-area.spatial.test.tsx` matching project naming) that:
  - Renders a grid with N rows × M columns.
  - Asserts each cell registers exactly one focus scope (no `GridCellFocusable` outer scope).
  - Asserts the registered rect's width/height matches the visible `<td>` border-box.
- [ ] Add a click-padding test: click on the padding area of a cell and assert the cell receives focus (use `getBoundingClientRect()` on the rendered `<td>` and dispatch a click at `rect.x + 2, rect.y + 2`).
- [ ] Run: `bun test field.spatial-nav.test.tsx` — green.
- [ ] Run: `bun test data-table` and `bun test grid-view` — green (no regressions).
- [ ] Run: `bun test` for the spatial-nav e2e suite — green.

## Workflow

- Use `/tdd` — failing test first (Field with padding registers larger rect), implement the prop, then collapse `GridCellFocusable`.
- After the prop lands, audit other Field call sites (Inspector, EntityCard, nav-bar, board-selector, column-view title) to confirm none accidentally need it — opt-in only.