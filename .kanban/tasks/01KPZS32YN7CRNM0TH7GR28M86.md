---
assignees:
- claude-code
depends_on:
- 01KNQXW7HHHB8HW76K3PXH3G34
- 01KQ5PP55SAAVJ0V3HDJ1DGNBY
position_column: doing
position_ordinal: '8480'
project: spatial-nav
title: 'Perspective: wrap perspective bar + container + view as zones, strip legacy nav'
---
## STATUS: REOPENED 2026-04-26 — does not work in practice

The user reports that **perspective tabs cannot be focused or selected**. Structural wrapping shipped, but clicking a perspective tab does not produce visible focus feedback. See umbrella card `01KQ5PEHWT...` for the systemic root-cause checklist.

## Remaining work

1. **Verify the click → indicator-rendered chain** for each perspective tab (`perspective_tab:{id}` leaves) inside the `ui:perspective-bar` zone.
2. The tabs are `<Focusable>` leaves wrapped via the conditional `PerspectiveTabFocusable`. Confirm the leaf renders a visible focus state, AND that `showFocusBar` default fires correctly through `<Focusable>`.
3. Verify `ui:perspective` and `ui:view` zones — do they need visible focus when the user drills out to them? If yes, design and add. If not, document why suppression is correct (these are viewport-sized chrome zones).
4. Audit the conditional zone-mount pattern (`PerspectiveBarSpatialZone`, `ViewSpatialZone`, `PerspectiveSpatialZone`): the conditional may correctly mount in production but be invisible in tests. Verify each branch in production with `bun tauri dev`.
5. Integration test: click a tab → assert visible focus indicator. Test arrow nav between tabs once `nav.right` is wired.

## Files involved

- `kanban-app/ui/src/components/perspective-tab-bar.tsx`
- `kanban-app/ui/src/components/perspective-container.tsx`
- `kanban-app/ui/src/components/view-container.tsx`
- `kanban-app/ui/src/components/focusable.tsx` and `focus-indicator.tsx`

## Acceptance Criteria

- [ ] Manual smoke: clicking a perspective tab shows a visible focus state on the tab
- [ ] Manual smoke: arrow keys between perspective tabs advance visible focus (when `nav.right` lands)
- [ ] Each container zone (`ui:perspective`, `ui:view`) with `showFocusBar={false}` has an inline comment explaining why
- [ ] Integration test asserts visible indicator after click on each perspective tab
- [ ] Existing perspective tests stay green

## Tests

- [ ] `perspective-tab-bar.spatial-nav.test.tsx` — click each tab → assert visible indicator
- [ ] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow

- Use `/tdd` — write the integration test first, watch it fail, then fix.

---

(Original description and prior implementation notes preserved below for reference.)

## (Prior) Implementation Note (2026-04-26)

Three components wrap themselves in spatial-nav primitives via the conditional-zone pattern (matches the established `BoardSpatialZone` / `SpatialZoneIfAvailable` shape so existing tests stay green when the spatial-nav stack is not mounted):

- `perspective-tab-bar.tsx` — `PerspectiveBarSpatialZone` wraps the tab bar root in `<FocusZone moniker="ui:perspective-bar" className="flex items-center border-b bg-muted/20 px-1 h-8 shrink-0">`. Each `<ScopedPerspectiveTab>` now goes through `PerspectiveTabFocusable`, which wraps the tab in `<Focusable moniker="perspective_tab:${id}">` when the spatial-nav stack is present.
- `perspective-container.tsx` — `PerspectiveSpatialZone` wraps `{children}` in `<FocusZone moniker="ui:perspective" className="flex flex-col flex-1 min-h-0 min-w-0">`.
- `view-container.tsx` — `ViewSpatialZone` wraps the active view in `<FocusZone moniker="ui:view" className="flex-1 flex flex-col min-h-0 min-w-0">`.

All 1498 tests passed at completion.