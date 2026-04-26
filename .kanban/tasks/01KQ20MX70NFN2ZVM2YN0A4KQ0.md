---
assignees:
- claude-code
depends_on:
- 01KNQXYC4RBQP1N2NQ33P8DPB9
position_column: doing
position_ordinal: '80'
project: spatial-nav
title: 'Column: wrap as zone, strip legacy keyboard nav from column-view'
---
## What

The structural part shipped — column body is wrapped in `<FocusScope kind="zone">`, registers correctly with the spatial-nav kernel, predicates removed, tests green. **But the user can't actually focus or select a column**: clicking a column does fire `spatial_focus`, but the visible feedback is suppressed by `showFocusBar={false}` on the column's FocusScope, so from the user's seat nothing happens.

There is plenty of clickable column whitespace (gutters around cards, empty tail below cards) — the click target is fine. The bug is **the focus indicator never renders for the column** because we explicitly disabled it.

## Files to fix

- `kanban-app/ui/src/components/column-view.tsx` — drop or revisit `showFocusBar={false}` on the column's FocusScope at line ~588

## Likely fix

Remove `showFocusBar={false}` from the column's `<FocusScope kind="zone">`. The original suppression was probably copied from the board/perspective/view container zones (which legitimately shouldn't paint a bar around the entire viewport). Columns are sized, distinct entities — they should advertise their focus the same way cards and field rows do.

If the default focus bar overlaps awkwardly with the column header chrome, address that with `<FocusIndicator>` positioning (left edge of the column box, full height) rather than by disabling the indicator.

## Verify drill-out works end to end

While we're here, confirm the Escape chain actually drills out from a card to its column zone (Rust nav.drillOut → `focus-changed` event → React claim → column re-renders with focused state). The drill-out card `01KPZS4RG0` claims this works but no integration test exercises card → column → board. Add one.

## Audit other container zones

Same `showFocusBar={false}` pattern likely applies to: `ui:board`, `ui:perspective`, `ui:view`, `ui:grid`, `ui:navbar`, each `ui:toolbar.*`. For each, decide deliberately:

- **Show the bar** — the zone is a sized, distinct entity (column, card, field row, navbar block, toolbar group)
- **Hide the bar with a code-comment justification** — the zone is viewport-sized and decorating it would be visually noisy

Document the decision inline.

## Subtasks
- [ ] Remove or revise `showFocusBar={false}` on the column FocusScope
- [ ] Reproduce manually: `bun tauri dev`, click a column body, confirm visible focus indicator appears on the column
- [ ] Add `column-view.spatial-nav.test.tsx` test: clicking the column body fires `spatial_focus` AND the column primitive's `data-focused` attribute appears after the kernel emits `focus-changed`
- [ ] Add integration test: card focused → Escape → column has `data-focused` → Escape → board zone has `data-focused`
- [ ] Audit `showFocusBar={false}` across every zone in the codebase; per-zone decision documented

## Acceptance Criteria
- [ ] Clicking a column produces a visible focus indicator on the column
- [ ] Pressing Escape from a focused card moves focus to the column AND shows the indicator
- [ ] Pressing Escape from a focused column moves focus to the board (or window root)
- [ ] Each container zone with `showFocusBar={false}` has an inline code comment explaining why
- [ ] An integration test covers the drill-out chain card → column → board → root
- [ ] Existing column-view tests still green

## Notes for the implementer

The lesson from the first pass: per-component cards passed because each tested "registration call wires correctly" rather than "user can navigate to this thing AND see they did." That's the wrong bar. Future zone-wrapping tests must include: deliberate click → visible feedback, AND deliberate Escape drill-out → visible feedback at the next level up.

## Workflow
- Use `/tdd` — write the integration test first (Escape from card lands on column with visible indicator), watch it fail, then fix.