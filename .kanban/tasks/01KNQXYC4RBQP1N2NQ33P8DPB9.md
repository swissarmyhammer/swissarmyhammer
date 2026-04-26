---
assignees:
- claude-code
depends_on:
- 01KNQXXF5W7G4JP73C6ZCMKYKX
- 01KQ4YYFCGJCRN6GBYGVGXVVG6
- 01KQ5PP55SAAVJ0V3HDJ1DGNBY
position_column: doing
position_ordinal: '8680'
project: spatial-nav
title: 'Inspector layer: one layer per window, panels and field rows as zones inside'
---
## STATUS: REOPENED 2026-04-26 — does not work in practice

The user reports that focus does not work inside inspector panels. The inspector layer + per-panel zone wrapping shipped, but the fields, labels, and pills inside cannot be focused or visibly selected. See umbrella card `01KQ5PEHWT...` for the systemic root-cause checklist.

## Remaining work

This card owns the inspector **layer / panel zone** wrapping. The leaf-level focus issues (labels, editors, pills) are owned by sibling card `01KNQY0P9J03...`. This card needs to confirm the layer + panel zones are doing their part:

1. **Verify the layer captures focus correctly.** With an inspector panel open, click on the panel body — does the panel zone receive focus and render visible feedback? If `showFocusBar={false}` is set on the panel zone, decide whether that's correct (the panel body fills with field rows, so a panel-edge bar might be the right affordance).
2. **Verify drill-out within the layer.** From a focused field row, Escape should land focus on the panel zone, then on (no parent — the layer pop). Walk this path manually.
3. **Verify multi-panel cross-zone fallback.** Open two panels, focus a field in panel B, close B → focus should land on panel A's `last_focused`. Walk this manually.

## Files involved

- `kanban-app/ui/src/components/inspectors-container.tsx`
- `kanban-app/ui/src/components/inspector-focus-bridge.tsx`

## Acceptance Criteria

- [ ] Manual smoke: panel zone is focusable and shows visible feedback (or has a documented reason for hiding the bar)
- [ ] Manual smoke: Escape from a field row inside a panel lands on the panel zone with visible feedback
- [ ] Manual smoke: closing panel B with focus inside it → focus lands on panel A's `last_focused`
- [ ] Integration test for panel-zone focus + drill-out chain
- [ ] Existing inspector tests stay green

## Tests

- [ ] `inspectors-container.spatial-nav.test.tsx` — panel zone receives focus + renders indicator
- [ ] Integration test for cross-panel `last_focused` fallback
- [ ] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow

- Use `/tdd` — write the integration test first, watch it fail, then fix.

---

(Original description and prior implementation notes preserved below for reference.)

## (Prior) Implementation Notes (2026-04-26)

`InspectorsContainer` reads `windowLayerKey = useCurrentLayerKey()` at the top, then wraps the panel list in `<FocusLayer name="inspector" parentLayerKey={windowLayerKey}>` only when `panelStack.length > 0`. Each panel is wrapped in `<FocusScope kind="zone" moniker="panel:${entityType}:${entityId}" showFocusBar={false}>` — the `panel:` moniker disambiguates from the underlying entity moniker. `useRestoreFocus()` removed; layer pop + zone `last_focused` handle that responsibility. New tests in `inspectors-container.test.tsx` (8 new) cover layer-mount lifecycle. New `inspectors-container.guards.node.test.ts` (4 tests) pins source-level invariants.