---
assignees:
- claude-code
depends_on:
- 01KQ7S6WHK9RCCG2R4FN474EFD
- 01KQ9XWHP2Y5H1QB5B3RJFEBBR
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffda80
project: spatial-nav
title: Perspective tabs must be focusable and arrow-navigable — visible indicator and Left/Right traversal among tab leaves
---
## What

The perspective tabs — `<FocusScope moniker="perspective_tab:{id}">` leaves inside a `<FocusZone moniker="ui:perspective-bar" showFocusBar={false}>` zone (`kanban-app/ui/src/components/perspective-tab-bar.tsx:434` for the leaf, `:283` for the zone) — are registered with the spatial graph but have two observable failures the user reports:

1. **No visible focus indicator on a focused perspective tab.** Even when spatial focus is on a tab, the `<FocusIndicator>` cursor-bar does not appear next to the tab's label.
2. **Arrow Left / Right does not traverse perspective tabs.** Beam search "right" from the leftmost tab should land on the next tab; "left" the symmetric path back. It doesn't.

This is structurally the same surface concern as the navbar work in `01KQ9XWHP2Y5H1QB5B3RJFEBBR` — same primitive shape (a `showFocusBar={false}` zone of horizontally laid-out `<FocusScope>` leaves), and both surfaces likely share root causes (rect timing, conditional re-mount on a child becoming "active", layer-scoped focus-claim filtering). This ticket depends on `01KQ9XWHP2Y5H1QB5B3RJFEBBR` so the navbar fix lands first; whatever the fix is, applying it to the perspective bar should be a small additional surface, and any primitive-level work covers both.

Cross-layer reachability (`Up` from a card / column to reach the perspective bar, `Down` from the navbar to it) is **not** part of this ticket — it is the unified-policy concern in `01KQ7S6WHK9RCCG2R4FN474EFD` (also depended on). Click-to-focus is **not** in scope: clicking a tab dispatches `perspective.set` (`perspective-tab-bar.tsx` `onSelect` handler), which switches the active perspective and re-renders the bar, so "click to focus and observe" is not a stable user flow. Indicator-visibility is verified via programmatic `setFocus` in tests, not via clicks.

## Surface specifics that differ from the navbar

The perspective bar has wrinkles the navbar does not, all worth pinning under test:

- **Active-tab inline chrome.** The active perspective tab renders extra inline content next to the `TabButton`: a `<FilterFocusButton>` and a `<GroupPopoverButton>` (`perspective-tab-bar.tsx:547–558`). These are NOT wrapped in their own `<FocusScope>` — they are inline buttons that dispatch via direct `onClick`. The active tab's `<FocusScope>` (the leaf moniker `perspective_tab:<id>`) wraps the entire `<div className="inline-flex items-center">` (`perspective-tab.tsx:535`), so the leaf's bounding rect grows when the tab becomes active. Beam search relies on that growing rect — verify it does not break the rect-based picks for sibling tabs.
- **Active-tab change does NOT remount the tab's `<FocusScope>`.** The `isActive` flag flips render content but not the wrapping leaf component. So no SpatialKey churn from tab activation, unlike the navbar's inspect-leaf `{board && ...}` conditional. Pin via test.
- **Add-perspective `+` button** (`AddPerspectiveButton`, `perspective-tab-bar.tsx:445`) is a sibling at the end of the tab list but is **not** wrapped in `<FocusScope>`. So it is not a beam-search candidate. This matches its role as a chrome-only affordance the user clicks rather than navigates to. Confirm the behavior is intentional and pin via a regression test asserting the `+` button is unreachable by `nav.right` from the rightmost tab — instead, `nav.right` falls through to the unified-policy drill out (or no-op, depending on whether `01KQ7S6WHK9RCCG2R4FN474EFD` has landed).
- **Inline rename editor.** When a tab is in rename mode (`isRenaming=true`, triggered by `ui.entity.startRename` Enter on the active tab), the `TabButton` renders `<InlineRenameEditor>` in place of the name. The editor takes DOM focus directly. Verify that during rename mode, the indicator state still flips correctly when the rename commits / cancels and the leaf re-claims focus.

## What's likely broken

The two seams mirror the navbar's seams (`01KQ9XWHP2Y5H1QB5B3RJFEBBR`):

### Seam 1 — Indicator not rendering on the leaf

The bar's layout (`PERSPECTIVE_BAR_LAYOUT`, defined elsewhere in the same file) is `pl-2 gap-2` per the docstring at line 214: "`pl-2` and `gap-2` are load-bearing — each tab is a `<FocusScope>`". The indicator paints `-left-2 top-0.5 bottom-0.5 w-1` outside the leaf's left edge, in the bar's `pl-2` (8 px) for the leftmost tab, or in the `gap-2` (8 px) between siblings.

Likely culprits:

- **Tooltip / Radix `asChild` cloning** on the tab buttons (similar to navbar). Verify the `<FocusScope>`'s `data-focused` flips and the indicator child renders.
- **Per-layer focus-claim filtering** when the inline rename editor is open. The editor takes DOM focus; the kernel may scope `focus-changed` to the active layer; the leaf's `useFocusClaim` is registered against the window layer.
- **Rect mismatch when the active tab grows.** When activation widens the active leaf's box (active inline chrome added), the leaf's `getBoundingClientRect()` updates only when the host's box changes — `ResizeObserver` should catch it. Pin via test.

### Seam 2 — Left/Right does not traverse perspective tabs

Same beam-search rect concern. With perspective tabs at horizontally-progressing rects within the `ui:perspective-bar` zone, `Direction::Right` from the leftmost tab should pick the next sibling. If today's behaviour is "Right does nothing", the most likely cause is stale or zero-sized rects at first paint. Pin with a Rust integration test against a realistic fixture (matching the navbar arrow-nav test) AND a browser-side regression that snapshots all tab leaves' rects at first paint and asserts none are zero-sized.

## Approach

### 1. Coordinate with `01KQ9XWHP2Y5H1QB5B3RJFEBBR`

Wait for the navbar fix to land first. Whatever primitive-level seam the navbar ticket addresses (Tooltip/asChild, per-layer claim filtering, rect timing, etc.) likely covers the perspective bar too. After the navbar fix, re-run the perspective-bar acceptance tests below — many or all may already pass.

### 2. Pin the perspective-bar-specific bugs

Whatever's left after the navbar fix is perspective-bar-specific. Likely candidates: active-tab rect growth, rename-editor focus-claim flip, the `+` button's deliberate non-reachability.

### 3. Fix at the seam the failing tests point at

Don't pre-emptively patch.

## Acceptance Criteria

All asserted by automated tests below — no manual smoke step.

- [x] When the kernel's `focused_key` is set to a perspective tab leaf's `SpatialKey`, the tab's `<FocusScope>` wrapper has `data-focused="true"` and a rendered `[data-testid="focus-indicator"]` child. Asserted for every visible perspective tab in the bar (not just the active one).
- [x] `nav.right` from the leftmost perspective tab lands focus on the next tab to its right.
- [x] `nav.right` walks rightward through every tab in turn until the rightmost.
- [x] `nav.right` from the rightmost tab is a no-op or, per the unified-policy dependency, drills out — but never bounces back.
- [x] `nav.left` walks the symmetric path back to the leftmost tab.
- [x] Activating a tab (clicking it or dispatching `perspective.set`) does NOT cause the focused leaf's SpatialKey to change. After activation, the tab's wrapper still has `data-focused="true"` (focus stays on the tab) and a rendered `[data-testid="focus-indicator"]` child. Regression guard for the active-tab inline-chrome rect growth.
- [x] Pressing Enter on the active perspective tab triggers `ui.entity.startRename` (existing scope-pinned binding); when rename commits or cancels, focus returns to the tab's leaf and the indicator re-renders. Regression guard for the rename round-trip.
- [x] The Add-perspective `+` button is NOT a beam-search candidate. `nav.right` from the rightmost tab does not land on the `+` button. (Pins the deliberate non-spatial nature of the `+` chrome.)
- [x] None of the perspective tabs' kernel-stored rects is zero-sized at first paint, including the active tab whose box is wider due to inline chrome. Regression guard.
- [x] No regression: clicking a tab still dispatches `perspective.set` and switches the active perspective; double-click still does NOT dispatch `ui.inspect` (perspectives are chrome — `perspective-tab-bar.no-inspect-on-dblclick.spatial.test.tsx`).

## Tests

All tests are automated. No manual verification.

### Frontend — `kanban-app/ui/src/components/perspective-tab-bar.focus-indicator.browser.test.tsx` (new file)

Mounts `<PerspectiveTabBar>` inside the production provider stack against the per-test backend with three perspectives (one active).

- [x] `focus_indicator_renders_when_inactive_tab_is_focused` — `spatial_focus(inactiveTabKey)`, await one tick, assert `[data-moniker="perspective_tab:<id>"][data-focused="true"]` exists with a `[data-testid="focus-indicator"]` descendant.
- [x] `focus_indicator_renders_when_active_tab_is_focused` — same for the active tab. Pins that the active-tab inline chrome (FilterFocus, GroupPopover) does not interfere with the indicator's containing block / overflow.
- [x] `focus_indicator_persists_through_tab_activation` — focus an inactive tab, dispatch `perspective.set` to make it the active one, await one tick, assert the same `<FocusScope>` wrapper still reports `data-focused="true"` and renders the indicator.
- [x] `focus_indicator_returns_after_rename_commit` — focus the active tab, dispatch `ui.entity.startRename` (which mounts `InlineRenameEditor`), commit the rename (Enter), await one tick, assert focus and the indicator return to the tab's `<FocusScope>` wrapper.
- [x] `focus_indicator_returns_after_rename_cancel` — same as above with Escape (cancel) instead of Enter.

Test command: `pnpm vitest run perspective-tab-bar.focus-indicator.browser` — all five pass.

### Rust kernel — `swissarmyhammer-focus/tests/perspective_bar_arrow_nav.rs` (new file)

Builds a realistic fixture (window-root layer, perspective-bar zone, three sibling leaves at horizontally-progressing rects mirroring the production layout, with the middle leaf wider to mirror an active tab). Reuses the fixture builder from `01KQ7STZN3G5N2WB3FF4PM4DKX` under `swissarmyhammer-focus/tests/fixtures/`.

- [x] `perspective_right_from_leftmost_tab_lands_on_middle_tab` — focused on tab 0, `BeamNavStrategy::next` with `Direction::Right` returns tab 1's moniker.
- [x] `perspective_right_from_middle_active_tab_lands_on_rightmost_tab` — middle tab is wider (active state); beam search still picks the next tab to the right by `left()` ordering.
- [x] `perspective_left_walks_symmetric_path` — starting from tab 2, `Direction::Left` returns tab 1, then tab 0.
- [x] `perspective_right_from_rightmost_tab_drills_out_to_perspective_bar` — coordinated with the unified-policy outcome (drills out to parent zone `ui:perspective-bar`, with no-bounce-back guard against previous tabs and the non-spatial `+` button).

Test command: `cargo test -p swissarmyhammer-focus --test perspective_bar_arrow_nav` — all four pass plus a fixture sanity tripwire.

### Frontend — augment existing tests

- [x] `kanban-app/ui/src/components/perspective-tab-bar.test.tsx` — add a regression test asserting that none of the perspective tabs' kernel-stored rects is zero-sized at first paint (including the active tab). Mounts the bar in the production provider stack and snapshots the rects via the spatial-focus actions.
- [x] `kanban-app/ui/src/components/perspective-tab-bar.no-inspect-on-dblclick.spatial.test.tsx` — pre-existing test must still pass. Confirmed.
- [x] `kanban-app/ui/src/components/perspective-tab-bar.enter-rename.spatial.test.tsx` — pre-existing tests for Enter→rename must still pass. Confirmed.

Test command: `pnpm vitest run perspective-tab-bar` — all pre-existing tests + the new rect regression all pass (66 tests across 8 files).

## Resolution

Both seams already worked in production thanks to the upstream navbar fix (kanban card `01KQ9XWHP2Y5H1QB5B3RJFEBBR`). All five new browser-mode focus-indicator tests, all four new Rust beam-search tests, and the new rect-regression test pass on first paint without any production-code changes. The tests are the regression guards so a future edit cannot silently break the wiring.

### Files added

- `swissarmyhammer-focus/tests/perspective_bar_arrow_nav.rs` (new) — five Rust integration tests pinning beam-search Right/Left through three sibling perspective tabs (with middle wider to mirror active-tab chrome), drill-out from rightmost to parent zone, no bounce-back, and a fixture-shape sanity tripwire.
- `kanban-app/ui/src/components/perspective-tab-bar.focus-indicator.browser.test.tsx` (new) — five browser-mode tests pinning `<FocusIndicator>` render on inactive + active tabs, persistence through tab activation, and return after rename commit + cancel.

### Files modified

- `swissarmyhammer-focus/tests/fixtures/mod.rs` — perspective bar's two placeholder leaves (`ui:perspective-bar.default` / `ui:perspective-bar.active`) replaced with three production-shaped leaves (`perspective_tab:p1`, `perspective_tab:p2`, `perspective_tab:p3`); p2 widened to 160 px (vs. p1/p3 at 96 px) to mirror the production active-tab inline chrome rect growth. Helpers `perspective_tab_p1_key()`, `perspective_tab_p2_key()`, `perspective_tab_p3_key()` added on `RealisticApp`.
- `kanban-app/ui/src/components/perspective-tab-bar.test.tsx` — appended a `describe("rect regression — first paint")` block mounting the bar inside the spatial-nav stack and asserting all three perspective tab leaves register non-zero rects via `spatial_register_scope`.

## Workflow

- Use `/tdd` — write the failing tests first against the production wiring. Many tests will fail in the same way the navbar tests do; the navbar fix from `01KQ9XWHP2Y5H1QB5B3RJFEBBR` may turn most of them green automatically when applied here.
- Single ticket — both seams (indicator + arrow nav) describe the perspective bar surface specifically. Cross-zone reachability (`Up` from card → perspective bar) is `01KQ7S6WHK9RCCG2R4FN474EFD`'s job. The navbar surface is `01KQ9XWHP2Y5H1QB5B3RJFEBBR`'s job. This card is the perspective-bar surface only.
- Land after the navbar work so any primitive-level fix is shared.