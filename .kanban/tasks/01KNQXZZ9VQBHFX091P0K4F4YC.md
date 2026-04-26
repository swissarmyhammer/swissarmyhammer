---
assignees:
- claude-code
depends_on:
- 01KNQXYC4RBQP1N2NQ33P8DPB9
position_column: doing
position_ordinal: '8880'
project: spatial-nav
title: 'Grid view: wrap as zone, strip legacy keyboard nav'
---
## What

Wrap the grid view in `<FocusZone moniker="ui:grid">`, register each row and cell appropriately, and strip every legacy keyboard-nav vestige from `grid-view.tsx` and `data-table.tsx`. The 2D grid is the cleanest case for spatial nav — cells are laid out in a regular XY grid, so nearest-neighbor by rect is exactly correct.

### Zone shape

```
ui:view (FocusZone, parent)
  ui:grid (FocusZone) ← THIS CARD
    grid_row:{i} (Leaf or Zone, depending on whether rows are themselves navigable units)
      grid_cell:{i,j} (Leaf, one per cell — task field display)
```

**Decision: rows are NOT zones, just visual groupings.** Cells are flat leaves under `ui:grid`. Spatial nav already handles row-and-column movement via beam search. Adding row-zones would force users into drill-in-then-drill-out for cross-row nav, which fights the natural grid UX.

If we later need row-level operations (select-row), we can promote rows to zones. For now: `ui:grid` is the only zone; cells are direct leaf children.

### Files to modify

- `kanban-app/ui/src/components/grid-view.tsx`
- `kanban-app/ui/src/components/data-table.tsx`

### Legacy nav to remove

- `buildCellPredicates()` function (~60 lines) in grid-view.tsx
- `claimPredicates` memo that calls `buildCellPredicates` for every cell
- `cellMonikerMap` (only used by predicates)
- `claimWhen` prop on `<DataTable>` columns and threading through cell rendering
- `ClaimPredicate` import in both files
- Any `onKeyDown` handlers on grid rows / cells / outer container
- Any `keydown` `useEffect` listeners scoped to the grid
- Any roving-tabindex implementation (replaced by spatial-key tracking on each leaf)

What stays: row-virtualization scroll handlers (mouse / scroll wheel), DnD row reorder if present.

### Subtasks
- [x] Wrap grid container in `<FocusZone moniker={Moniker("ui:grid")}>`
- [x] Each cell becomes a `<Focusable moniker={Moniker(`grid_cell:${rowIndex}:${colKey}`)}>` leaf with `parent_zone = ui:grid`
- [x] Delete `buildCellPredicates`
- [x] Delete `claimPredicates` memo and `cellMonikerMap`
- [x] Remove `claimWhen` threading through DataTable cell rendering
- [x] Remove `ClaimPredicate` imports in grid-view.tsx and data-table.tsx
- [x] Remove all `onKeyDown` / `keydown` handlers from both files
- [x] Update or remove tests that assert on `buildCellPredicates`

## Acceptance Criteria
- [x] Grid view registers exactly one Zone (`ui:grid`) at its root, with `parent_zone = ui:view`
- [x] Every cell registers as a Leaf with `parent_zone = ui:grid`
- [x] `buildCellPredicates` deleted; ~60 lines removed
- [x] No `claimWhen` / `ClaimPredicate` / `onKeyDown` / `keydown` references in grid-view.tsx or data-table.tsx
- [ ] Grid cell navigation works identically via spatial nav: up/down/left/right move between cells; `nav.first` / `nav.last` go to top-left / bottom-right; `nav.rowStart` / `nav.rowEnd` operate on the cell's row (via the same-row filter in the algorithm card) — **deferred** to follow-up `01KNQY1GQ9ABHEPE0NJW909EAQ` (arrow-key spatial nav wiring)
- [x] `pnpm vitest run` passes

## Tests
- [x] `grid-view.spatial-nav.test.tsx` — grid container is a Zone; cells are leaves with the grid zone as parent_zone
- [x] `grid-spatial-nav.guards.node.test.ts` — no `claimWhen` / `cellMonikers` / `claimPredicates` props in data-table.tsx; no `onKeyDown` / `keydown` references in either file
- [x] `grid-view.cursor-ring.test.tsx` — existing cursor-ring suppression still passes (regression); plus a new click-to-cursor regression that pins the entity-focus update on left-click in the spatial path
- [x] `grid-view.nav-is-eventdriven.test.tsx` — existing fetch-discipline contract still passes (regression)
- [x] `data-table.test.tsx` — cell rendering doesn't accept legacy props; new test asserts the runtime stand-in
- [x] Run `cd kanban-app/ui && npx vitest run` — all 1539 pass (140 files), `tsc --noEmit` clean

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

## Implementation Notes (2026-04-26)

- `grid-view.tsx` now wraps the non-empty grid body in a `<GridSpatialZone>` helper that conditionally mounts `<FocusZone moniker={asMoniker("ui:grid")}>` only when both `useOptionalLayerKey()` and `useOptionalSpatialFocusActions()` resolve. Mirrors the `BoardSpatialZone` / `ViewSpatialZone` / `PerspectiveSpatialZone` pattern so existing GridView unit tests keep their narrow provider tree without forcing them to mount `<SpatialFocusProvider>` + `<FocusLayer>`.
- `useGridNavigation` lost ~80 lines: `buildCellPredicates`, `orthogonalNavPredicates`, `rowEdgeNavPredicates`, `gridEdgeNavPredicates`, `useCellMonikers`, `cellMonikerMap`, and the `claimPredicates` memo are all gone. Cursor derivation is now a single `resolveCursorFromFocus(focusedMoniker, columns, rowCount)` helper that parses `grid_cell:R:K` directly — no field-moniker-map lookup required. Initial focus seeds `gridCellMoniker(0, columns[0].field.name)` via `useInitialCellFocus`.
- `data-table.tsx` lost the `cellMonikers`, `claimPredicates`, and `useClaimNav` props/flag entirely. The `GridCellScope` (FocusScope-based) was replaced with `GridCellFocusable` which conditionally wraps cell content in `<Focusable moniker={asMoniker(gridCellMoniker(di, colKey))}>`. The conditional mirrors the GridSpatialZone pattern — when the spatial stack is absent (bare `<DataTable>` test harness), the cell renders without the `<Focusable>` wrapper to avoid `useCurrentLayerKey` throwing.
- New `grid-spatial-nav.guards.node.test.ts` (18 tests) pins the source-level invariants: no `ClaimPredicate` / `claimWhen` / `cellMonikers` / `claimPredicates` / `buildCellPredicates` / `cellMonikerMap` / `onKeyDown` / `keydown` tokens, plus positive assertions for `<FocusZone moniker={asMoniker("ui:grid")}>`, `Focusable` import, `gridCellMoniker` import, and `asMoniker(gridCellMoniker(...))` brand application.
- New `grid-view.spatial-nav.test.tsx` (4 tests) mounts GridView in the production-shaped provider stack and asserts: (1) exactly one `spatial_register_zone` call with `moniker="ui:grid"`, (2) the `data-moniker="ui:grid"` element is in the DOM, (3) every cell registers as a `spatial_register_focusable` with the correct `grid_cell:R:K` moniker for its position, (4) every cell focusable's `parentZone` equals the grid zone's key.
- Updated `grid-view.nav-is-eventdriven.test.tsx`'s `DataTable` mock to drop the `cellMonikers` prop access (the prop no longer exists). Added a runtime stand-in test in `data-table.test.tsx` that asserts the table mounts without the legacy props.
- The arrow-key spatial nav wiring acceptance criterion is **deferred** to follow-up task `01KNQY1GQ9ABHEPE0NJW909EAQ` — that card pins the keymap (ArrowUp/Down/Left/Right → spatial nav) once all zones / leaves are registered. This card established the registration contract; the keymap binds against it next.

## Review Findings (2026-04-26 09:42) — RESOLVED 2026-04-26 09:55

### Warnings
- [x] `kanban-app/ui/src/components/data-table.tsx` (`GridCellFocusable`) — In production with the spatial stack mounted, `<Focusable>`'s `onClick` calls `e.stopPropagation()` (per the long-standing FocusScope convention). The result: the surrounding `<TableCell onClick={onClick}>` handler — which calls `handleCellClick(di, ci)` → `setFocus(gridCellMoniker(...))` via entity-focus — does NOT fire in the spatial path. So clicking a cell now updates only the spatial-focus key (Rust side); the entity-focus moniker (`focusedMoniker`) the cursor ring derives from is not touched. Behavior delta vs the pre-change code where click moved the cursor. Acceptable as a transition state (this card defers arrow-key nav to `01KNQY1GQ9...`), but should be tracked. Fix path: bridge `focus-changed` Tauri events back into entity-focus (resolve `next_key` → moniker, call `setFocus`), or merge `handleCellClick` into the `<Focusable>` click chain via a new prop. Either way, the legacy entity-focus side needs an event-driven update once spatial focus is the canonical click target.
  - **Resolution**: `GridCellFocusable` in the spatial path now mounts a thin `<div onClick onDoubleClick>` *inside* `<Focusable>` (mirroring `FocusScopeBody` in `focus-scope.tsx`). React's bubble order runs the inner wrapper's handler before `Focusable.onClick` calls `stopPropagation()`, so on a left-click both sides fire: `Focusable` updates spatial focus (Rust side) AND the inner `onClick` runs `handleCellClick(di, ci)` → `setFocus(gridCellMoniker(...))` so the cursor ring (derived from entity-focus) tracks click-to-move-cursor in production again. Fallback path (no spatial stack) keeps handlers on the `<TableCell>` as before. Pinned by a new regression test in `grid-view.cursor-ring.test.tsx` ("clicking a cell sets entity-focus and lights the cursor ring on that cell") that fails on the pre-fix code and passes on the fix.
- [x] `kanban-app/ui/src/components/data-table.tsx` (`GridCellFocusable` docstring, "We attach our own `onClick` / `onDoubleClick` to the surrounding `<TableCell>` so left-click drives the legacy grid-cursor `handleCellClick` callback (in addition to the spatial focus call)") — Docstring claims left-click drives both spatial focus and the legacy `handleCellClick`. As shown above, `Focusable.onClick`'s `stopPropagation()` shadows the TableCell's onClick handler in the spatial path, so `handleCellClick` does NOT run on left-click in production. Either fix the comment to describe actual behavior (spatial-focus only on click; double-click still enters edit mode) or change the wiring so both handlers actually fire.
  - **Resolution**: Docstring rewritten to describe the new wiring per branch (spatial path wraps a div inside `<Focusable>` so the inner handler fires before Focusable's `stopPropagation`; fallback path attaches handlers directly to `<TableCell>`). The behaviour now matches the doc — both spatial focus AND legacy `handleCellClick` fire on a left-click in production.

### Nits
- [x] `kanban-app/ui/src/components/grid-view.tsx` (`useGridNavigation` return value) — `hasFocusedMoniker: focusedMoniker !== null` is returned but never consumed anywhere in the codebase (verified: zero matches outside the producer). Dead code; drop it from the return shape.
  - **Resolution**: dropped from the return shape.
- [x] `kanban-app/ui/src/components/data-table.tsx` (`GridCellFocusableProps`) — Interface declares `ci` and `isCursor` props but the function body renames them to `_ci` / `_isCursor` (unused). Remove both from the interface and from the call site in `DataBodyCell` (the `ci={ci}` and `isCursor={isCursor}` JSX attributes); `cursorRef` already encodes "is cursor" via the `isCursor ? props.cursorRef : undefined` selector at the call site.
  - **Resolution**: removed `ci` and `isCursor` from the interface, function signature, and the `DataBodyCell` call site. `cursorRef` continues to encode "is cursor" via the existing `isCursor ? props.cursorRef : undefined` selector.
- [x] `kanban-app/ui/src/components/grid-view.tsx` (`useGridNavigation`) — Both `derivedCursor: {row, col}` and `gridCellCursor: {row, colKey}` are derived from the same `focusedMoniker`. `parseGridCellMoniker` already returns `{row, colKey}`; the current code goes `moniker → {row, col} (via columns.findIndex) → {row, colKey} (via columns[col].field.name)`. Consider deriving `gridCellCursor` directly from `parseGridCellMoniker`'s output (and keeping `derivedCursor` only for `useGrid`'s numeric-cursor input). Minor — single render path, but removes a redundant column-index round-trip.
  - **Resolution**: `gridCellCursor` now derives directly from `parseGridCellMoniker`'s output (with row-range and column-existence guards inline). `derivedCursor` keeps its original column-index-based derivation since `useGrid` needs the numeric `{row, col}` shape. The two reads are independent and each does the minimum work for its consumer.