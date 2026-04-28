---
assignees:
- claude-code
depends_on:
- 01KQ5PP55SAAVJ0V3HDJ1DGNBY
- 01KQ5QB6F4MTD35GBTARJH4JEW
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffca80
project: spatial-nav
title: 'RELEASE BLOCKER: spatial-nav focus is not actually visible to users — verify every component end-to-end'
---
## What

The spatial-nav project shipped registration plumbing across every component but **the user cannot focus or visibly select**:
- columns
- cards
- card titles, status, pills
- inspector field rows
- inspector field labels and pills
- nav bar buttons
- perspective tabs

Every per-component card was marked `done` because its unit tests verified "registration call fired correctly." None of them verified "user can deliberately focus this element AND see that they focused it." That's the wrong bar for a UX-facing system. This card is the umbrella that pins the systemic verification step that must pass before we can call any of the per-component cards genuinely complete.

## Likely root causes (to investigate, not assume)

The breakage is one or more of:

1. **`showFocusBar={false}` applied liberally to container zones.** Many zones (column, board, perspective, view, navbar, etc.) pass `showFocusBar={false}` to suppress the indicator. For viewport-sized zones (board, perspective, view) this is correct — a focus bar around the whole viewport is noise. For sized entities (column, navbar group, inspector panel) it is wrong — the user has no idea that focus landed there.

2. **`FocusIndicator` not rendering or positioned wrong on leaves.** Even when `showFocusBar=true`, the indicator may be invisible because of CSS positioning (e.g. clipped by `overflow:hidden`, behind a higher-z-index sibling, sized to zero, etc.).

3. **Claim subscription not firing.** `useFocusClaim(key, setFocused)` should re-render the primitive when the Rust kernel emits `focus-changed` for that key. If the React-side claim registry isn't receiving the event (or the key resolution is wrong), the primitive's `focused` state stays `false` and the indicator never appears.

4. **Focus-changed event not emitted by Rust on `spatial_focus`.** The Tauri command `spatial_focus(key)` is supposed to update `focus_by_window` and emit `focus-changed`. If the emit is gated, conditional, or miswired, the React side never wakes.

5. **Wrong key in the claim registry.** Each primitive mints a `SpatialKey` via `useRef`. If the registration uses one key but the claim subscription uses a different key (e.g. parent zone's key by accident), focus events for the registered key won't notify the right callback.

## Verification protocol — every component card must pass this

For each component (column, card, title leaf, status leaf, pill leaf, field row zone, field label leaf, field pill leaf, nav bar zone, nav bar leaf, perspective tab leaf):

1. **Manual smoke**: `cd kanban-app && bun tauri dev`, click on the component, confirm a visible focus indicator appears on it.
2. **Integration test**: render the component in production-shaped provider stack, simulate a click, advance through the Rust → focus-changed → React claim → setState path, assert the rendered DOM has the focus indicator visible (not just `data-focused`, but the actual `<FocusIndicator>` element rendered).
3. **Drill-out test (where applicable)**: focus a child leaf, dispatch `nav.drillOut`, assert the parent zone gets the focus indicator.

## Ownership

This card does not do the fixing — that work happens in the per-component cards (now reopened):

- `01KNQXZ81Q` Board view
- `01KQ20MX70` Column
- `01KQ20NMRQ` Card (entity-card)
- `01KNQXZZ9V` Grid view
- `01KQ20Q2PN` NavBar
- `01KQ20QW3K` Toolbar groups
- `01KPZS32YN` Perspective
- `01KNQXYC4RB` Inspector layer
- `01KNQY0P9J` Inspector and badge-list

This card is the gate: **the spatial-nav project remains incomplete until all of those per-component cards demonstrate visible focus on their own subject AND a green integration test exercises the click → render path.**

## Acceptance Criteria

- [ ] Manual smoke: every listed component shows a visible focus indicator when clicked
- [ ] Manual smoke: Escape from a card → the column shows visible focus
- [ ] Manual smoke: Escape from a column → the board / window root shows visible focus or transitions cleanly
- [ ] One integration test per component asserts the click → claim → indicator-rendered chain
- [ ] One integration test asserts the drill-out chain (card → column → board) shows visible focus at each step
- [ ] Any zone that legitimately suppresses the indicator (viewport-sized chrome) has an inline code comment explaining why
- [ ] All per-component cards in the list above moved back to `done` only after their own acceptance criteria include the manual smoke + integration test items

## Workflow

This is a coordination card. Pick up the per-component cards individually via `/implement`. Each component's card description has been updated with the specific remaining work and acceptance criteria.