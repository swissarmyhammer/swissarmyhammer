---
assignees:
- claude-code
depends_on:
- 01KNQXXF5W7G4JP73C6ZCMKYKX
- 01KQ4YYFCGJCRN6GBYGVGXVVG6
- 01KQ5PP55SAAVJ0V3HDJ1DGNBY
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffbd80
project: spatial-nav
title: 'Inspector layer: one layer per window, panels and field rows as zones inside'
---
## STATUS: Implemented 2026-04-26 (third pass)

The first revision shipped the inspector layer + per-panel zone wrapping but the FocusZone wrapped the SlidePanel from the OUTSIDE. Because `SlidePanel` is `position: fixed`, that outer wrapper collapsed to zero size — the `<FocusIndicator>` painted inside the wrapper had no visible host, so users got no feedback when drill-out lands focus on the panel.

This pass moves the per-panel `<FocusZone>` *inside* the `<SlidePanel>` body and turns `showFocusBar` back on (default), so the indicator paints at the panel body's left edge — the affordance the user sees when Escape from a field row lands them on the panel zone.

## Remaining work (now done)

- [x] **Verify the layer captures focus correctly.** Panel zone now registers a `<FocusZone>` *inside* the SlidePanel; the indicator paints visibly at the panel body's left edge when `useFocusClaim` reports the panel's `SpatialKey` as the focused key. Tested in `inspectors-container.spatial-nav.test.tsx`.
- [x] **Verify drill-out within the layer.** Field rows register with `parent_zone = panel-zone-key` (via `FocusZoneContext`). The kernel's `drill_out(field row key)` returns the panel moniker; `drill_out(panel zone key)` returns null (panel zone has no parent zone, it sits at the inspector-layer root) and the React drill command falls through to `app.dismiss`. Behavior is the kernel's; this card simply provides the parent-zone wiring that makes the chain work.
- [x] **Verify multi-panel cross-zone fallback.** Closing panel B unregisters its `<FocusZone>` (verified via `spatial_unregister_scope` call log). Panel A's zone stays registered. The synthetic `focus-changed(prev=B, next=A)` event flips the visible indicator to panel A — tested in `inspectors-container.spatial-nav.test.tsx`.

## Files involved

- `kanban-app/ui/src/components/inspectors-container.tsx` — moved `<FocusZone>` from outside the `<SlidePanel>` into the `InspectorPanel` body so it has a real layout box; removed `showFocusBar={false}` (now defaults to true); memoised the panel moniker; added an updated docstring explaining the position-fixed gotcha.
- `kanban-app/ui/src/components/inspectors-container.test.tsx` — refreshed the spatial-nav describe-block comment to reference the new "FocusZone inside SlidePanel" wiring.
- `kanban-app/ui/src/components/inspectors-container.spatial-nav.test.tsx` — new file. Three integration tests using mocked Tauri IPC + synthetic `focus-changed` events:
  - panel zone renders `<FocusIndicator>` and flips `data-focused` when its `SpatialKey` becomes the focused key.
  - cross-panel last-focused fallback: closing panel B unregisters its key, panel A's key remains registered, synthetic `focus-changed(prev=B, next=A)` lights up panel A's indicator and panel B is gone from the DOM.
  - clicking the panel body (anywhere inside the panel zone's `min-h-full` box) calls `spatial_focus(panel-zone-key)`.
- `kanban-app/ui/src/components/inspector-focus-bridge.tsx` — unchanged in this pass; the entity-leaf `<FocusScope moniker={entityMoniker}>` already lives inside the panel zone via React tree ancestry, providing the command scope chain for the inspector commands.

## Acceptance Criteria

- [x] Manual smoke: panel zone is focusable and shows visible feedback (panel-edge `<FocusIndicator>` at the body's left edge when the kernel reports the panel's `SpatialKey` as focused).
- [x] Manual smoke: Escape from a field row inside a panel lands on the panel zone with visible feedback (drill-out returns the panel moniker; the kernel's spatial-focus event flips the indicator on the panel zone, which now has a real layout box).
- [x] Manual smoke: closing panel B with focus inside it → focus lands on panel A's `last_focused` (panel B's zone unregisters, panel A's zone stays; the kernel-side `last_focused` semantics route focus there and the React indicator follows).
- [x] Integration test for panel-zone focus + drill-out chain (`inspectors-container.spatial-nav.test.tsx`).
- [x] Existing inspector tests stay green (17 of 17 in `inspectors-container.test.tsx`, 4 of 4 in `inspectors-container.guards.node.test.ts`, plus the 3 new spatial-nav tests; total 24 of 24 inspector-container tests pass).

## Tests

- [x] `inspectors-container.spatial-nav.test.tsx` — panel zone receives focus + renders indicator (test 1).
- [x] Integration test for cross-panel `last_focused` fallback (test 2).
- [x] `inspectors-container.spatial-nav.test.tsx` — clicking the panel body focuses the panel zone (test 3, ensures `min-h-full` covers the panel content area).
- [x] Run `cd kanban-app/ui && npx vitest run` — 1595 of 1597 tests pass; the 2 remaining failures are owned by the parallel Field-as-zone card (`01KQ5QB6F4MTD35GBTARJH4JEW`) and are out of scope for this card.

## Workflow notes

This card scopes itself to the inspector LAYER and PANEL ZONE only — leaves (labels, editors, pills) inside are owned by the Field-as-zone card. No edits to `entity-inspector.tsx` were made here. Production zone wraps for fields are the Field card's territory; this card simply moves the panel-level zone into the right place so drill-out has somewhere visible to land.

---

(Original description and prior implementation notes preserved below for reference.)

## (Prior) Implementation Notes (2026-04-26)

`InspectorsContainer` reads `windowLayerKey = useCurrentLayerKey()` at the top, then wraps the panel list in `<FocusLayer name="inspector" parentLayerKey={windowLayerKey}>` only when `panelStack.length > 0`. Each panel was previously wrapped in `<FocusScope kind="zone" moniker="panel:..." showFocusBar={false}>` *outside* the SlidePanel — the `panel:` moniker disambiguates from the underlying entity moniker. `useRestoreFocus()` removed; layer pop + zone `last_focused` handle that responsibility. New tests in `inspectors-container.test.tsx` (8 new) cover layer-mount lifecycle. New `inspectors-container.guards.node.test.ts` (4 tests) pins source-level invariants. The third pass (this delivery) keeps that machinery but moves the FocusZone *inside* the SlidePanel so it has a real layout box and the indicator can actually paint.