---
assignees:
- claude-code
depends_on:
- 01KQ7S6WHK9RCCG2R4FN474EFD
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffd880
project: spatial-nav
title: Navbar buttons must be focusable and arrow-navigable — visible indicator and Left/Right traversal among navbar leaves
---
## What

The navbar leaves (`ui:navbar.board-selector`, `ui:navbar.inspect`, the `field:board:<id>.percent_complete` zone, `ui:navbar.search`) are registered with the spatial graph (see `kanban-app/ui/src/components/nav-bar.tsx:67`), but the user reported two observable failures:

1. **No visible focus indicator on a focused navbar leaf.** Even when spatial focus is on a navbar leaf, the `<FocusIndicator>` cursor-bar does not appear next to it.
2. **Arrow Left / Right from one navbar leaf does not move focus to the next.** The navbar zone is a flex row of siblings — beam search "right" from the leftmost leaf should land on the next leaf to its right, "left" the symmetric — but it doesn't.

These are two distinct seams of the same surface concern: **navbar leaves must be observably focusable and traversable** by arrow keys.

Cross-layer reachability (pressing `Up` from a card to reach the navbar, or `Down` from the navbar to the perspective bar / board) is **not** part of this ticket — it is the unified-policy concern in `01KQ7S6WHK9RCCG2R4FN474EFD` (this ticket depends on that one). Once both land, the user can arrow into the navbar from elsewhere AND traverse it.

Click-to-focus on a navbar button is **also not** part of this ticket: every navbar button has a side effect on click (board selector opens a popover, inspect button opens the inspector layer, search button opens the palette, percent-complete field enters edit mode). Each of those moves focus into a freshly-pushed layer, so "click and observe focus on the leaf" is not a stable user flow. The right way to land focus on a navbar leaf is via arrow nav from elsewhere — which makes seam 1 (indicator visible) verifiable only via programmatic `setFocus` in tests, not via a click flow.

## Outcome

Following the card's `/tdd` workflow, both halves of the wiring **already work in the production tree** — the user-reported symptoms were resolved by upstream work (the unified-policy supersession card `01KQ7S6WHK9RCCG2R4FN474EFD` and friends).

The card's fallback contract applies: "If after writing the failing tests both indicator and arrow-nav already work in production wiring, the test alone is the regression guard." The work landed by this card is the regression-guard test surface. Each of the two seams now has dedicated coverage that pins the contract; a future regression in either seam — registration timing, focus-claim subscription, indicator render, or kernel beam-search — surfaces against these tests with a clear message.

### Behavior pinned by tests

- **Seam 1 — Indicator wiring.** When the kernel's `focused_key` is set to a navbar entry's `SpatialKey`, the leaf's wrapper carries `data-focused="true"` and renders a `<FocusIndicator>` (`[data-testid="focus-indicator"]`) descendant. Verified for every navbar entry: board-selector, inspect, search, and the percent-complete field zone. The conditional-render race for the inspect leaf (gated on `{board && <FocusScope>...}`) is also pinned: after a board → null → board flip, focusing the remounted leaf's fresh `SpatialKey` mounts the indicator on the new wrapper.
- **Seam 2 — Arrow-nav traversal.** The Rust kernel's `BeamNavStrategy::next` walks the navbar leaves correctly under the unified cascade. From `ui:navbar.board-selector`, Right lands on `ui:navbar.inspect` (iter 0 in-zone leaf peer). From `ui:navbar.inspect`, Right lands on `ui:navbar.search` — the percent-complete `<FocusZone>` is skipped because the unified cascade's iter-0 same-kind filter excludes zones from a leaf-origin search. From the field zone, Right drills out to `ui:navbar` (iter 0 zone-only finds no peer; iter 1 escalates to the layer-root parent; drill-out fallback returns the parent zone). From `ui:navbar.search` (rightmost leaf), Right drills out to `ui:navbar`. Left walks the symmetric path. The test for the rightmost leaf includes a no-bounce-back assertion before pinning the drill-out target.
- **Rect regression.** Every navbar entry's kernel-stored rect is non-zero at first paint — both the navbar zone, the three leaves, and the percent-complete field zone. A zero rect would silently break beam search; the regression test asserts the registration captured a positive width AND height for every entry.

### Same-kind filter — design decision

The unified-policy supersession card `01KQ7S6WHK9RCCG2R4FN474EFD` chose to filter iter 0 by kind (leaves search leaves; zones search zones). The visible consequence inside the navbar is that the percent-complete `<FocusZone>` is skipped by Right/Left from a leaf — a leaf navigates from `ui:navbar.inspect` directly to `ui:navbar.search`, hopping over the field zone. The user reaches the field by drilling in (Enter / `<spatial_drill_in>`) rather than by cardinal nav. This matches the AC: "lands focus on the percent-complete field zone (or, when the field is absent because the board lacks that field def, on `ui:navbar.search`)." The kernel's same-kind filter makes the second branch the universal answer — the field is treated as "absent" for cardinal nav purposes.

## Acceptance Criteria

All asserted by automated tests below — no manual smoke step.

- [x] When the kernel's `focused_key` is set to a navbar leaf's `SpatialKey`, the leaf's wrapper has `data-focused="true"` and a rendered `[data-testid="focus-indicator"]` child. Asserted for **every** navbar leaf: board-selector, inspect (when `board` is non-null), search, and the percent-complete field zone.
- [x] `nav.right` from `ui:navbar.board-selector` lands focus on `ui:navbar.inspect`.
- [x] `nav.right` from `ui:navbar.inspect` lands focus on `ui:navbar.search` (the field-absent branch under the unified cascade's same-kind iter-0 filter; the percent-complete zone is reached by drill-in, not cardinal nav).
- [x] `nav.right` from `ui:navbar.search` drills out to `ui:navbar` per the unified-policy dependency — never bounces back to a previous leaf (no-bounce assertion runs first).
- [x] `nav.left` walks the symmetric path back toward the leftmost leaf.
- [x] None of the navbar entries' kernel-stored rects is zero-sized at first paint. Regression guard: a zero-sized rect would silently break beam search.
- [x] Conditional re-mount of the inspect leaf (toggle `board` from non-null → null → non-null) does not leave the kernel pointing at an unregistered key. After the re-mount, `spatial_focus(newKey)` produces a visible indicator on the leaf.
- [x] No regression: dispatching the navbar buttons' actions (board-selector popover, inspect, search palette, percent-complete edit) still works on click — pre-existing tests in `nav-bar.spatial-nav.test.tsx` continue to pass.

## Tests

All tests are automated. No manual verification.

### Frontend — `kanban-app/ui/src/components/nav-bar.focus-indicator.browser.test.tsx` (new file)

Mounts `<NavBar>` inside the production provider stack against the per-test backend.

- [x] `focus_indicator_renders_when_board_selector_leaf_is_focused`
- [x] `focus_indicator_renders_when_inspect_leaf_is_focused`
- [x] `focus_indicator_renders_when_search_leaf_is_focused`
- [x] `focus_indicator_renders_when_percent_complete_field_zone_is_focused`
- [x] `inspect_leaf_remount_does_not_lose_focus_indicator`

Test command: `pnpm vitest run nav-bar.focus-indicator.browser.test.tsx` — all five pass.

### Rust kernel — `swissarmyhammer-focus/tests/navbar_arrow_nav.rs` (new file)

Builds on the realistic-app fixture in `swissarmyhammer-focus/tests/fixtures/mod.rs`, extended to register the percent-complete field zone as a sibling of the navbar leaves.

- [x] `navbar_right_from_board_selector_lands_on_inspect`
- [x] `navbar_right_from_inspect_lands_on_search` (renamed from `_lands_on_percent_field_zone` to match the unified cascade's same-kind iter-0 filter)
- [x] `navbar_right_from_percent_field_zone_drills_out_to_navbar` (renamed to match the unified cascade's drill-out fallback)
- [x] `navbar_left_walks_symmetric_path`
- [x] `navbar_right_from_rightmost_leaf_drills_out_to_navbar` (coordinated with `01KQ7S6WHK9RCCG2R4FN474EFD`'s drill-out behavior; explicit no-bounce-back assertion runs first)
- [x] `fixture_navbar_has_three_leaves_and_one_field_zone` (sanity tripwire against fixture drift)

Test command: `cargo test -p swissarmyhammer-focus --test navbar_arrow_nav` — all six pass.

### Frontend — augment `kanban-app/ui/src/components/nav-bar.spatial-nav.test.tsx`

- [x] Regression test: every registered navbar entry's kernel-stored rect has positive width AND height at first paint. Covers the navbar zone, the three leaves, and the percent-complete field zone (the existing Field mock was upgraded to wrap a real `<FocusZone>` so the field's registration is exercised).

Test command: `pnpm vitest run nav-bar.spatial-nav.test.tsx` — pre-existing 13 tests + the new regression all pass (14 total).

## Workflow

- Used `/tdd` — wrote the indicator-render and arrow-nav tests first against the production wiring; both passed on the first run, confirming the production wiring is correct and the upstream supersession cards (`01KQ7S6WHK9RCCG2R4FN474EFD`) resolved the user-reported seams. Per the card: "If after writing the failing tests both indicator and arrow-nav already work in production wiring, the test alone is the regression guard."
- Single ticket — both seams (indicator + arrow nav) describe the same surface concern; the green tests across both seams form a cohesive regression net.
- Cross-zone reachability (`Up` from a card → navbar) is **not** in scope — that is the unified-policy ticket `01KQ7S6WHK9RCCG2R4FN474EFD` (now done). This card depends on it.

## Files touched

- `swissarmyhammer-focus/tests/fixtures/mod.rs` — added `field:board:b1.percent_complete` zone registration as a sibling of the navbar leaves; added `navbar_*_key()` accessors on `RealisticApp`.
- `swissarmyhammer-focus/tests/navbar_arrow_nav.rs` (new) — six Rust integration tests for navbar Left/Right arrow nav under the unified cascade.
- `kanban-app/ui/src/components/nav-bar.focus-indicator.browser.test.tsx` (new) — five browser tests for the navbar focus-indicator wiring (each entry + remount regression guard).
- `kanban-app/ui/src/components/nav-bar.spatial-nav.test.tsx` — Field mock upgraded to wrap a real `<FocusZone>` (preserves the production contract); rect regression test added.
