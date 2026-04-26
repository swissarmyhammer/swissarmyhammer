---
assignees:
- claude-code
depends_on:
- 01KNQXW7HHHB8HW76K3PXH3G34
- 01KQ5PP55SAAVJ0V3HDJ1DGNBY
- 01KQ5QB6F4MTD35GBTARJH4JEW
position_column: doing
position_ordinal: '8280'
project: spatial-nav
title: 'NavBar: wrap as zone, strip legacy keyboard nav'
---
## STATUS: REOPENED 2026-04-26 — does not work in practice

The user reports that **nav bar buttons cannot be focused or selected**. The structural wrapping shipped (zone + leaves), but clicking a nav bar button does not produce visible focus feedback. See umbrella card `01KQ5PEHWT...` for the systemic root-cause checklist.

## Remaining work

1. **Verify the click → indicator-rendered chain** for each nav bar leaf:
   - `ui:navbar.board-selector`
   - `ui:navbar.inspect`
   - `ui:navbar.search`
2. The navbar buttons are `<Focusable>` leaves — confirm they render a visible `<FocusIndicator>` when their `SpatialKey` is the focused key for the window. If `<FocusIndicator>` doesn't fit visually inside a button (e.g. it's clipped or invisible against the button background), redesign that leaf's focus state — could be an outline, ring, or inverted background instead of the cursor-bar.
3. Audit `ui:navbar` zone-level focus: does the zone itself need a visible indicator when the user drills out to it? If yes, design it; if no, document why and keep `showFocusBar={false}`.
4. Add an integration test per leaf and per zone-level focus.

## Files involved

- `kanban-app/ui/src/components/nav-bar.tsx`
- `kanban-app/ui/src/components/focus-indicator.tsx` (verify it works inside a button)
- `kanban-app/ui/src/components/focusable.tsx` (default `showFocusBar` + render path)

## Acceptance Criteria

- [ ] Manual smoke: clicking the board selector shows a visible focus state on it
- [ ] Manual smoke: clicking the inspect button shows a visible focus state on it
- [ ] Manual smoke: clicking the search button shows a visible focus state on it
- [ ] Manual smoke: arrowing left/right between nav bar buttons (after focusing one) advances visible focus
- [ ] Integration test per nav bar leaf asserts the visible indicator after click
- [ ] Each leaf with `showFocusBar={false}` has an inline comment explaining why

## Tests

- [ ] `nav-bar.spatial-nav.test.tsx` — click each leaf → assert visible indicator
- [ ] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow

- Use `/tdd` — write the integration test first (click each leaf → indicator visible), watch it fail, then fix.

---

(Original description and prior implementation notes preserved below for reference.)

## (Prior) Implementation Note (2026-04-25)

The `<header>` was replaced with `<FocusZone moniker="ui:navbar">` keeping all of its layout classes; three actionable children — `BoardSelector`, the inspect button, the search button — are wrapped in `<Focusable>` leaves with `ui:navbar.board-selector`, `ui:navbar.inspect`, and `ui:navbar.search` monikers. The inspect leaf is conditional on `board` so it only registers when its content is actually rendered. The percent-complete `Field` is intentionally NOT wrapped — it's a composite that owns its own focus model.

`role="banner"` was added back so the landmark role survives the swap from `<header>` to `<FocusZone>` rendering a `<div>`. All 1499 vitest tests passed at completion.