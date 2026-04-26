---
assignees:
- claude-code
depends_on:
- 01KNQXYC4RBQP1N2NQ33P8DPB9
- 01KQ5PP55SAAVJ0V3HDJ1DGNBY
position_column: doing
position_ordinal: '8780'
project: spatial-nav
title: 'Board view: wrap as zone, strip legacy keyboard nav'
---
## STATUS: REOPENED 2026-04-26 — does not work in practice

The user reports that the broader spatial-nav system (column, card, etc.) doesn't actually let them focus or select. The board-zone wrapping shipped, but it's the root of all the per-component breakage. See umbrella card `01KQ5PEHWT...` for the systemic root-cause checklist.

## Remaining work

1. **Audit the board zone's `showFocusBar` setting** and verify it's the correct decision. The board fills the viewport, so a focus bar around the entire board body would be visually noisy — `showFocusBar={false}` is probably correct here. Document the decision inline.
2. **Verify drill-out lands on the board.** From a focused column, Escape should land focus on the board zone, then on the window root layer. Even though the bar is hidden, the focus state should still be present (data-focused attribute, last_focused stored). Walk this manually.
3. **Verify the `useInitialBoardFocus` hook** seeds focus correctly on board mount. The user should land somewhere visible when the board first loads.
4. Integration test: drill-out from column → board zone has data-focused (even without visible indicator).

## Files involved

- `kanban-app/ui/src/components/board-view.tsx`

## Acceptance Criteria

- [ ] Manual smoke: opening the app lands focus on a visible element (first card, or first column header) per `useInitialBoardFocus`
- [ ] Manual smoke: Escape from a focused column reaches the board zone (data-focused present, even if no visible indicator)
- [ ] Manual smoke: Escape from the board zone reaches the window root layer cleanly
- [ ] `showFocusBar={false}` on board zone has an inline comment explaining the viewport-size suppression rationale
- [ ] Integration test exercises the drill-out chain card → column → board → window root
- [ ] Existing board-view tests stay green

## Tests

- [ ] `board-view.spatial-nav.test.tsx` — drill-out chain reaches board zone
- [ ] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow

- Use `/tdd` — write the integration test first, watch it fail, then fix.

---

(Original description and prior implementation notes preserved below for reference.)

## (Prior) Round 2 Implementation Notes (2026-04-26)

All four review findings addressed: `BoardView` JSDoc rewritten to describe the spatial-nav zone model; `useInitialBoardFocus` JSDoc rewritten; `BoardSpatialZone` got a named `BoardSpatialZoneProps` interface; `useColumnTaskMonikers` simplified to `useInitialFocusMoniker`. 1553 tests pass; tsc clean.