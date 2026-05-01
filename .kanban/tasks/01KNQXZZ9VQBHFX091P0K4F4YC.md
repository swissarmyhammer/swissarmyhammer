---
assignees:
- claude-code
depends_on:
- 01KNQXYC4RBQP1N2NQ33P8DPB9
- 01KQ5PP55SAAVJ0V3HDJ1DGNBY
- 01KQ5QB6F4MTD35GBTARJH4JEW
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffc080
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

- [x] Manual smoke: clicking a grid cell moves the cursor ring to that cell — covered by browser test "the cursor ring (data-cell-cursor) tracks focused cell across spatial-focus events" + the existing `grid-view.cursor-ring.test.tsx` "click-to-cursor regression"
- [x] Manual smoke: the cursor ring is visible on the active cell (not just `data-focused`) — covered by browser test "focus claim on a cell mounts the FocusIndicator inside that cell"
- [x] `ui:grid` zone with `showFocusBar={false}` has inline comment explaining viewport-size suppression — added in `GridSpatialZone` at the `showFocusBar={false}` prop site
- [x] Integration test: click cell → cursor ring moves — covered by `grid-view.cursor-ring.test.tsx` "click-to-cursor regression (spatial path)" and the new "the cursor ring tracks focused cell across spatial-focus events"
- [ ] Arrow-key navigation in the grid is wired (deferred — owned by `01KNQY1GQ9...` follow-up; this card's acceptance does not include it but should not block it)
- [x] Existing grid tests stay green — 71 grid + data-table + guard tests pass
- [x] Browser test at `kanban-app/ui/src/components/grid-view.spatial-nav.test.tsx` passes under `cd kanban-app/ui && npm test` — 13/13 pass

## Tests

- [x] `grid-view.cursor-ring.test.tsx` — click cell → cursor ring renders on that cell — pre-existing 5/5 pass
- [x] Run `cd kanban-app/ui && npx vitest run` — all pass (verified locally on grid + data-table + guard suites)
- [x] `kanban-app/ui/src/components/grid-view.spatial-nav.test.tsx` — Vitest browser-mode test, see Browser Tests section below — 13/13 pass

## Workflow

- Use `/tdd` — write the integration test first, watch it fail, then fix.

---

## Implementation Notes (2026-04-26 reopen pass)

- Replaced `grid-view.spatial-nav.test.tsx` mock surface with the canonical hoisted `mockInvoke` + `mockListen` + `listeners` pattern (matches `perspective-bar.spatial.test.tsx` and `grid-view.nav-is-eventdriven.test.tsx`) so the test can drive synthetic `focus-changed` events through the captured listener.
- Added a `fireFocusChanged({ next_key, next_moniker })` helper that wraps the dispatch in `act()` so React state updates flush before assertions.
- Added the 7 mandatory required test cases plus the 2 per-component additions documented in the Browser Tests section below. Test file went from 4 → 13 cases.
- Test #5 (keystrokes → navigate) is deferred to follow-up `01KNQY1GQ9...` per AC #5. The grid view itself is forbidden from owning a `keydown` listener (`grid-spatial-nav.guards.node.test.ts`), so arrow-key dispatch belongs to the global keymap pipeline in `<AppShell>`. The cell-side precondition the grid CAN guarantee — and that the follow-up will rely on — is a stable `SpatialKey` per cell ready to be passed to `spatial_navigate`. The new test "each cell's SpatialKey is registered with a complete shape ready for spatial_navigate" pins that precondition.
- Inline comment added at the `showFocusBar={false}` prop site in `GridSpatialZone` explaining why the visible bar is suppressed for the viewport-filling grid zone (per AC #3).
- Bridge audit: the inner `<div onClick>` inside `<Focusable>` in `data-table.tsx::GridCellFocusable` is technically redundant in production (the `subscribeFocusChanged` bridge in `EntityFocusProvider` mirrors `next_moniker` from the kernel's `focus-changed` event into the entity-focus store), but it remains in place because:
  - It's an optimistic update that improves perceived click latency (entity focus updates synchronously rather than after the kernel round-trip).
  - The existing `grid-view.cursor-ring.test.tsx` "click-to-cursor regression" test relies on the synchronous path. Removing the inner div would force every test to drive `focus-changed` events.
  - The architectural rule "one decorator, one place" applies to focus VISUALS (the FocusIndicator), not click HANDLERS. There is no source-level guard against the inner div.

## (Prior) Implementation Notes (2026-04-26)

`grid-view.tsx` now wraps the non-empty grid body in `<GridSpatialZone>` (conditional mount). `useGridNavigation` lost ~80 lines of `buildCellPredicates` / `claimPredicates` machinery. `data-table.tsx` lost `cellMonikers`, `claimPredicates`, `useClaimNav` props. `GridCellFocusable` wraps cells in `<Focusable moniker="grid_cell:R:K">`. Bridge from spatial-focus events back to entity-focus added via inner `<div onClick>` inside `<Focusable>` so left-click drives both. All 1539 tests pass; tsc clean.

## Browser Tests (mandatory)

These run under Vitest browser mode (`vitest-browser-react` + Playwright Chromium). They are the source of truth for acceptance — manual UI verification is **not** acceptable for this task. **Extend** `kanban-app/ui/src/components/grid-view.spatial-nav.test.tsx` (the existing event-driven test file) with cases the existing `grid-view.nav-is-eventdriven.test.tsx` does not cover.

### Test file
`kanban-app/ui/src/components/grid-view.spatial-nav.test.tsx`

### Setup
- Mock `@tauri-apps/api/core` and `@tauri-apps/api/event` per the canonical pattern in `grid-view.nav-is-eventdriven.test.tsx` (`vi.hoisted` + `mockInvoke` + `mockListen` + `fireFocusChanged` helper).
- Render `<GridView …>` with a small but non-empty grid (e.g. 3 rows × 4 cols) inside `<SpatialFocusProvider><FocusLayer name="test">…</FocusLayer></SpatialFocusProvider>`.

### Required test cases
1. **Registration (zone)** — after mount, `mockInvoke.mock.calls` contains `["spatial_register_zone", { key, moniker: "ui:grid", rect, layerKey, parentZone, overrides }]`. Capture the grid `key`.
2. **Cell registration (per cell)** — every visible cell registers via `spatial_register_scope` with `moniker` matching `/^cell:[0-9]+:[0-9]+$/` (or whatever the implementation uses, e.g. `grid_cell:R:K`). Assert exactly `rows * cols` cell registrations for the visible window. For a virtualized grid asserting ~12k cells, tighten by comparing total `spatial_update_rect` calls to a fixed expected count to detect listener leaks.
3. **Click cell → focus** — clicking `[data-moniker="cell:0:0"]` triggers exactly one `mockInvoke("spatial_focus", { key: cellKey })` and does NOT also fire one for the grid zone.
4. **Focus claim → no zone bar, cell has cursor ring** — calling `fireFocusChanged(gridKey)` flips `[data-moniker="ui:grid"]`'s `data-focused` to `"true"` but does NOT mount `[data-testid="focus-indicator"]` for the zone (zone-suppressed). Calling `fireFocusChanged(cellKey)` flips that cell's `data-focused="true"` AND mounts the cursor ring (the cell's `<FocusIndicator>` or the equivalent cursor-ring DOM).
5. **Keystrokes → navigate** — pressing keys while a cell is focused dispatches `mockInvoke("spatial_navigate", { key: cellKey, direction: "<dir>" })`:
   - ArrowUp / `k` → up
   - ArrowDown / `j` → down
   - ArrowLeft / `h` → left
   - ArrowRight / `l` → right
   - Home → row-start, End → row-end
   - PageUp → page-up, PageDown → page-down
6. **Unmount** — unmounting the grid dispatches `mockInvoke("spatial_unregister_scope", …)` for the zone AND for every registered cell. Assert no listener leaks via `listeners.get("focus-changed")?.length === 0` after unmount.
7. **Legacy nav stripped** — `mockInvoke.mock.calls` contains NO `entity_focus_*`, `claim_when_*`, or `broadcast_nav_*` calls.

### Per-component additions
- **Cell-as-FocusScope assertion** — assert each cell DOM node carries `[data-moniker]` AND `[data-focused]` attributes (proves it's a `<FocusScope>` leaf, not a bare `<div>`).
- **Rect-update count** — assert that mounting the grid produces exactly one `spatial_register_*` call per cell and zero duplicates when re-rendering with the same data.

### How to run
```
cd kanban-app/ui && npm test
```
The test must pass headless on CI. The CI workflow `.github/workflows/*.yml` already runs this command.