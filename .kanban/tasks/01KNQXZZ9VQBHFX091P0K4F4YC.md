---
assignees:
- claude-code
depends_on:
- 01KNQXYC4RBQP1N2NQ33P8DPB9
- 01KQ5PP55SAAVJ0V3HDJ1DGNBY
- 01KQ5QB6F4MTD35GBTARJH4JEW
position_column: doing
position_ordinal: '8880'
project: spatial-nav
title: 'Grid view: wrap as zone, strip legacy keyboard nav'
---
## STATUS: REOPENED 2026-04-26 — does not work in practice

The user reports that grid cells cannot be focused or selected. Registration shipped, but the visible cursor / focus indicator on a grid cell is missing or not driven by spatial focus. See umbrella card `01KQ5PEHWT...` for the systemic root-cause checklist.

## Remaining work

The grid view already has a `cursor` ring (the existing visual selection ring around the active cell). With spatial nav now driving focus, that cursor ring needs to derive from `focused-key` events, not from a separate `useGrid` numeric cursor. The closeout note acknowledges this is partially in place (`gridCellCursor` derives from `parseGridCellMoniker(focusedMoniker)`), but:

1. **Verify the cursor ring tracks `focusedMoniker` end-to-end.** Click a cell → does the cursor ring move? Use ArrowLeft/Right/Up/Down — does the cursor ring follow? (Note: arrow-key spatial nav is deferred to `01KNQY1GQ9...`, so for this card focus on click-to-cursor only.)
2. **Verify the bridge from spatial-focus events to entity-focus.** The closeout note added an inner `<div onClick>` inside `<Focusable>` so left-click sets both spatial-focus AND entity-focus (which the cursor ring derives from). Confirm this still works in production (`bun tauri dev`) — test mocks may pass even if the production path is broken.
3. **Zone-level focus on `ui:grid`.** The grid is a viewport-filling zone; `showFocusBar={false}` is probably correct. Document why inline.

## Files involved

- `kanban-app/ui/src/components/grid-view.tsx`
- `kanban-app/ui/src/components/data-table.tsx`
- `kanban-app/ui/src/components/focusable.tsx`

## Acceptance Criteria

- [ ] Manual smoke: clicking a grid cell moves the cursor ring to that cell
- [ ] Manual smoke: the cursor ring is visible on the active cell (not just `data-focused`)
- [ ] `ui:grid` zone with `showFocusBar={false}` has inline comment explaining viewport-size suppression
- [ ] Integration test: click cell → cursor ring moves
- [ ] Arrow-key navigation in the grid is wired (deferred — owned by `01KNQY1GQ9...` follow-up; this card's acceptance does not include it but should not block it)
- [ ] Existing grid tests stay green

## Tests

- [ ] `grid-view.cursor-ring.test.tsx` — click cell → cursor ring renders on that cell
- [ ] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow

- Use `/tdd` — write the integration test first, watch it fail, then fix.

---

(Original description and prior implementation notes preserved below for reference.)

## (Prior) Implementation Notes (2026-04-26)

`grid-view.tsx` now wraps the non-empty grid body in `<GridSpatialZone>` (conditional mount). `useGridNavigation` lost ~80 lines of `buildCellPredicates` / `claimPredicates` machinery. `data-table.tsx` lost `cellMonikers`, `claimPredicates`, `useClaimNav` props. `GridCellFocusable` wraps cells in `<Focusable moniker="grid_cell:R:K">`. Bridge from spatial-focus events back to entity-focus added via inner `<div onClick>` inside `<Focusable>` so left-click drives both. All 1539 tests pass; tsc clean.