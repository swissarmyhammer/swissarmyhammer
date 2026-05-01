---
assignees:
- claude-code
position_column: todo
position_ordinal: 9f80
project: spatial-nav
title: 'FIX: column zone is unfocusable in practice — no click target, no visible feedback'
---
## What

User-reported regression after the spatial-nav project was marked done: "I cannot focus or select a column."

The column-as-zone work (`01KQ20MX70`) is technically wired but has two compounding problems that make it look broken from the user's seat:

1. **No visible feedback when a column is focused.** `column-view.tsx` passes `showFocusBar={false}` on the column's `<FocusScope kind="zone">`. So even when focus *does* land on the column zone (via drill-out or whitespace click), the user sees nothing.

2. **No clickable target for the column itself in normal layouts.** The column body fills with `<ColumnHeader>` + `<VirtualizedCardList>`. Both stop click propagation when they have a focused child. The only way to land a click on the column zone is to click empty whitespace below the last card inside the virtualized list — and the column's `showFocusBar={false}` then masks the result.

3. **Drill-out path unverified.** `nav.drillOut` (Escape, card `01KPZS4RG0`) should send focus from a card → its parent zone (the column) → the board zone → root. We need to confirm that path actually fires for column focus and that the column's `last_focused` survives the round trip.

This is a UX-level failure even if every per-component card passed its tests in isolation.

## Files to investigate / modify

- `kanban-app/ui/src/components/column-view.tsx` — column FocusScope's `showFocusBar={false}` is the primary culprit
- `kanban-app/ui/src/components/focus-indicator.tsx` (or wherever the bar renders) — verify the indicator can sit at the column's left edge without overlapping the header
- `kanban-app/ui/src/components/focus-zone.tsx` and `focusable.tsx` — audit the click handler chain; verify the column's onClick is reachable and that `e.stopPropagation()` on children doesn't accidentally swallow legitimate column-focus clicks
- `kanban-app/ui/src/lib/spatial-focus-context.tsx` — confirm `focus-changed` events for the column key actually update the React-side claim and trigger a re-render of the column zone

## Likely fix

a) **Re-enable `showFocusBar` on the column zone.** A focus bar at the column's left edge is exactly the affordance the user expects. The original "showFocusBar=false" decision was probably about avoiding a noisy bar around the entire viewport-sized board zone — that argument doesn't apply to columns. Columns are sized, distinct entities; they should advertise their focus.

b) **Give the column zone a real click target.** Two options:
   - Make the entire column header draggable-strip *also* a column-zone click target (header still focuses the name leaf, but when the click is on the header chrome and not the title text, it routes to the column zone).
   - Add a thin "column gutter" — a 4-8px focus strip at the column's left edge that intercepts clicks and focuses the column zone. Doubles as the focus-bar location.

c) **Verify drill-out lands on the column.** Add an integration test for: focus a card → press Escape → assert column is now focused → press Escape → assert board is focused → press Escape → assert window root.

## Subtasks
- [ ] Reproduce the failure: `bun tauri dev`, click on a column body, verify no focus-bar appears anywhere on the column
- [ ] Decide on showFocusBar: turn it back on for the column zone and re-test the visual
- [ ] Decide on click target: is the column-gutter approach worth the design work, or is "click on column whitespace inside the card list" enough?
- [ ] Verify drill-out from a card actually reaches the column zone (Rust → focus-changed → React claim wakes the column)
- [ ] Audit other container zones for the same `showFocusBar={false}` blindspot: `ui:board`, `ui:perspective`, `ui:view`, `ui:grid`, `ui:navbar`, `ui:toolbar.*`. Each needs a deliberate decision: "we hide the bar because X", or "we should be advertising focus here."
- [ ] Add integration test exercising Escape-driven drill-out across the full chain

## Acceptance Criteria
- [ ] Clicking on a column produces a visible focus indicator on the column itself
- [ ] Pressing Escape from a focused card moves focus to its column and shows the column's focus indicator
- [ ] Pressing Escape from a focused column moves focus to the board (or window root)
- [ ] Every container zone has either a visible focus indicator OR a documented "this zone never advertises focus, here's why" code comment
- [ ] An integration test covers the drill-out chain card → column → board → root

## Tests
- [ ] `column-view.test.tsx` — clicking the column body fires `spatial_focus` for the column moniker
- [ ] `column-view.test.tsx` — when the column moniker is the focused key for the window, the column renders a focus indicator
- [ ] Integration: Escape from a card fires `nav.drillOut` and the column's `data-focused` attribute appears
- [ ] Integration: Escape from a column fires `nav.drillOut` and reaches the board zone

## Notes for the implementer

This is the kind of issue that escapes per-component unit tests because each card was treated as "is the registration call wired correctly" rather than "can a user actually navigate to this thing." Future zone-wrapping cards should include an explicit acceptance criterion: **"the user can deliberately focus this zone via either click or Escape drill-out, AND see that they did."**

## Workflow
- Use `/tdd` — write the integration test first (Escape from card lands on column with visible indicator), watch it fail, then fix.