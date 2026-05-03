---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffc80
project: spatial-nav
title: Left from leftmost perspective tab lands on no visible focus instead of LeftNav
---
## What

Reported behavior: with focus on the leftmost `perspective_tab:{id}` leaf in the perspective tab bar, pressing `ArrowLeft` (`nav.left`) leaves the user with no visible focus indicator anywhere on screen. The visibly-leftward `<LeftNav>` sidebar is never reached.

Sample log:
```
2026-05-03 07:06:30.662681-0500   command  args=Some(Object {"scope_chain": Array [
  String("perspective_tab:01KPCRANPEWSSD89ZY7VGS5BNQ"),
  String("perspective:01KPCRANPEWSSD89ZY7VGS5BNQ"),
  String("ui:perspective-bar"),
  String("board:board"),
  String("store:/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/.kanban"),
  String("mode:normal"),
  String("window:board-01kqdzgz26ejbrdg2h9nxce6te"),
  String("engine"),
]}) board_path=None cmd=ui.setFocus scope_chain=Some(["engine"]) target=None
```

The result `scope_chain=Some(["engine"])` and `target=None` indicates focus collapsed to the engine root rather than landing on a sibling. The visible app layout has `<LeftNav>` (`ui:left-nav` zone) to the LEFT of the perspective bar — the user's expectation is for `Left` to land on a focusable element inside that sibling zone.

### Layout context

`kanban-app/ui/src/App.tsx` mounts the chrome as a vertical column under `<FocusLayer name="window">`:

```
<NavBar />                          ← ui:navbar (top, full-width)
<ViewsContainer>                    ← injects <LeftNav /> as a flex sibling
  <LeftNav />                       ← ui:left-nav (vertical sidebar, visibly LEFT)
  <PerspectivesContainer>
    <PerspectiveContainer>
      <PerspectiveTabBar />         ← ui:perspective-bar (row to the RIGHT of LeftNav)
      <ViewContainer />             ← ui:view (board / grid below the bar)
```

`ui:perspective-bar`, `ui:left-nav`, and `ui:navbar` are all peer zones at the layer root (`/window/ui:*`). Their `parent_zone` in the spatial registry is the layer root.

### Expected cascade per `swissarmyhammer-focus/README.md`

Cardinal `Left` from `perspective_tab:p1` (leftmost tab inside `ui:perspective-bar`) should run:

1. **Iter 0** — any-kind in-zone peer search inside `ui:perspective-bar`: no left peer of `p1` → miss.
2. **Iter 1** — same-kind peer-zone search: `ui:perspective-bar`'s peer zones at the layer root include `ui:navbar`, `ui:left-nav`, `ui:view`. `ui:left-nav` is geometrically left of `ui:perspective-bar` and should win the beam score → return `ui:left-nav` (or a leaf inside it).
3. **Drill-out fallback** — if iter 1 also misses, return the parent zone (the layer root), which would mean staying near `ui:perspective-bar` itself.

None of those should produce `scope_chain=["engine"]` / `target=None`. Either the cascade is failing entirely on this path, or it is returning a zone whose `showFocusBar={false}` (`ui:perspective-bar` and `ui:left-nav` both set this — see perspective-tab-bar.tsx:315 and left-nav.tsx:56) so the user observes "no visible focus" even though `data-focused` may be flipping on a wrapper. The latter is consistent with the user's complaint "leads to no visible focus" but does not explain the `scope_chain=["engine"]` result.

### Two candidate root causes (investigate, then fix)

1. **Cascade fails to find `ui:left-nav` at iter 1.** Possible if the peer-zone beam score requires Y overlap and `ui:left-nav` (a vertically tall sidebar starting just below the navbar) has only partial Y overlap with the perspective bar's row. Or the cascade is short-circuiting to a stay-put / clear-focus path. Add a regression test against the realistic-app fixture extended with a `ui:left-nav` zone (currently absent from `swissarmyhammer-focus/tests/fixtures/mod.rs`) and pin the expected destination.
2. **Cascade lands on a `showFocusBar={false}` zone with no follow-through.** When iter 1 returns a sibling zone whose visible indicator is suppressed, the user sees no focus. The kernel should either (a) drill IN to the destination zone's natural leaf in the search direction (e.g. for `Left`, the rightmost / topmost leaf inside `ui:left-nav`), or (b) the consuming app should not suppress the indicator on cross-zone landings. Option (a) is the kernel-side fix and matches the user's mental model.

### Files to read first

- `swissarmyhammer-focus/src/navigate.rs` — `BeamNavStrategy::next` and the iter-0 / iter-1 cascade. Verify the iter-1 implementation actually escalates to the parent zone's peer zones and applies beam scoring.
- `swissarmyhammer-focus/tests/fixtures/mod.rs` (line ~410 onwards) — the realistic-app fixture currently registers `ui:navbar` and `ui:perspective-bar` but **not** `ui:left-nav`. Extend it to mirror production layout so the regression test has something to land on.
- `swissarmyhammer-focus/tests/perspective_bar_arrow_nav.rs` — currently covers in-zone Left/Right between tabs and `Right` drill-out from the rightmost tab, but has no `Left` from the leftmost tab.
- `kanban-app/ui/src/components/perspective-tab-bar.tsx:300–321` — `PerspectiveBarSpatialZone` with `showFocusBar={false}`.
- `kanban-app/ui/src/components/left-nav.tsx:54–68` — `<LeftNav>` with `showFocusBar={false}`.

### Likely fix shape

Pick whichever the investigation supports:

- If iter 1 is missing `ui:left-nav` due to beam scoring, fix the scoring (or its candidate filter) in `swissarmyhammer-focus/src/navigate.rs`.
- If iter 1 returns `ui:left-nav` but the user sees nothing, extend the cascade to drill into the destination zone's natural leaf (rightmost leaf for `Left`, leftmost for `Right`, bottom for `Up`, top for `Down`). The drill-in should land on a leaf with `showFocusBar=true` so the indicator paints.

Whatever the fix, the result must be a `FullyQualifiedMoniker` that produces a visible `<FocusIndicator>` on screen — never `target=None`.

## Acceptance Criteria

- [x] In a layout with `ui:navbar` + `ui:left-nav` + `ui:perspective-bar` peer zones (matching the production tree), `BeamNavStrategy::next(registry, perspective_tab:p1, segment, Direction::Left)` returns a `FullyQualifiedMoniker` whose path lies within `ui:left-nav` (the zone itself or a leaf inside it). It MUST NOT return the focused FQM (stay-put), the layer root, or an unrelated zone.
- [x] In the running app, pressing `ArrowLeft` while focus is on the leftmost perspective tab moves visible focus into the LeftNav (the user observes a `<FocusIndicator>` paint on a view button or zone). The `ui.setFocus` IPC must carry a non-`None` `target` and a `scope_chain` with at least one `ui:left-nav`-anchored frame above `engine`.
- [x] No regression: existing `perspective_bar_arrow_nav.rs` tests (in-zone left/right walks, `Right` drill-out from rightmost tab) still pass.
- [x] No regression in other spatial-nav suites — run the full `cargo test -p swissarmyhammer-focus`.

## Tests

- [x] Extend `swissarmyhammer-focus/tests/fixtures/mod.rs` `RealisticApp` to register a `ui:left-nav` zone with the production geometry: positioned to the left of `ui:perspective-bar`, vertically spanning from below the navbar down to the bottom of the window. Add at least two `view:{id}` leaves inside it (mirroring the `ScopedViewButton` shape in `kanban-app/ui/src/components/left-nav.tsx`). Expose accessor methods `left_nav_fq()`, `view_button_grid_fq()`, etc.
- [x] In `swissarmyhammer-focus/tests/perspective_bar_arrow_nav.rs`, add `#[test] fn perspective_left_from_leftmost_tab_crosses_to_left_nav()` that asserts `nav(perspective_tab:p1, Left)` returns either `left_nav_fq()` itself or one of its leaves — and explicitly asserts it does NOT return the focused FQM, the layer root FQM, or `ui:perspective-bar` / `ui:navbar` / `ui:view`.
- [x] Add `#[test] fn perspective_left_from_leftmost_tab_never_collapses_to_layer_root()` as a defensive regression that asserts the result is not `app.layer_root_fq()` and not `app.engine_root_fq()` (whichever the fixture exposes).
- [x] Run `cargo test -p swissarmyhammer-focus perspective_bar_arrow_nav` and confirm the new tests pass and the existing ones stay green.
- [x] Run `cargo test -p swissarmyhammer-focus` to catch any cross-test regressions if the cascade behavior changes.

## Workflow

- Use `/tdd` — extend the realistic-app fixture with `ui:left-nav`, write the failing `perspective_left_from_leftmost_tab_crosses_to_left_nav` regression test (RED) against the current cascade, then either fix iter 1's beam scoring or add cross-zone drill-in until the test passes (GREEN). Confirm no other test in the kernel suite regresses.

## Review Findings (2026-05-03 14:47)

### Warnings
- [x] `swissarmyhammer-focus/README.md` — README's "The cascade" section is now stale wrt the new contract. The README still describes iter 1 as returning the destination zone's FQM (e.g. "Down from `tags-zone` (the bottom-most child of the card) → the next card below in the column, because `task:T1A`'s peer zones at `column:TODO`'s level include `task:T2A`"). The implemented kernel now drills into the destination zone's natural child after iter 1 succeeds — the visible result is a leaf, not the destination zone. The `navigate.rs` module docstring already documents this, but the README is the canonical prose contract (the kernel's own comment says "See `swissarmyhammer-focus/README.md` for the prose contract"). Add a fourth step or extend step 2 to describe the cross-zone drill-in: when iter 1 lands on a sibling zone, the cascade descends into that zone's natural child in the search direction (rightmost for `Left`, leftmost for `Right`, bottom for `Up`, top for `Down`) so the returned FQM identifies a leaf the focus indicator can paint on. Update the existing `Down`-from-`tags-zone` example to land on the next card's natural-`Down` child (its title leaf) rather than the card zone itself.

### Nits
- [x] `swissarmyhammer-focus/src/navigate.rs` `drill_into_natural_leaf` — the comment "Echo the input — the caller already produced a valid FQM" is slightly misleading when the destination is a registered leaf (rather than unregistered). In practice iter 1 only returns sibling zones so the leaf branch is unreachable from the cascade, but if a future caller passes a leaf the function happily echoes it. Consider tightening the comment to "Destination isn't a zone (or isn't registered); the caller already produced a valid FQM, so echo it" to make explicit that "isn't a zone" is a real-but-unreachable-from-cascade branch.

## Reviewer notes (no findings, for context)

- **Termination of `drill_into_natural_leaf`:** safe. Recursion is gated on `child_is_zone`, each step descends one level in the zone tree, and the registry forbids zone cycles.
- **Edge cases:** empty zone → returns destination zone unchanged; destination not a zone → echoes input; nested zones → bounded recursion; non-overlapping rects → `pick_natural_child` uses pure extreme-edge ordering with documented tie-breakers, no rect-intersection requirement.
- **Test fidelity:** the 9 updated tests across `card_directional_nav.rs`, `navigate.rs`, and `unified_trajectories.rs` faithfully assert the new contract — every old "lands on destination zone" assertion is replaced with a specific natural-child leaf FQM (e.g. `leaf1_fq`, `col_b_name_fq`, `label_2_fq`, `col0_card_b_title`, `column_name_fq`, `perspective_tab_p1_fq`, `navbar_board_selector_fq`). None silently weakened. Test names were renamed to reflect the new behavior (e.g. `cross_zone_right_lands_on_next_column_zone` → `cross_zone_right_drills_into_next_column_leftmost_leaf`).
- **Beam-search consistency:** natural-child selection (rightmost for `Left`, leftmost for `Right`, bottom for `Up`, top for `Down`) is consistent with the README's beam-search semantics — it mirrors the "extreme edge in the opposite of `direction`" geometry the beam score itself prefers, with documented tie-breakers (topmost for horizontal, leftmost for vertical) matching production reading order.
- **Scope question (universal vs. layer-root chrome only):** universal drill-in is the right contract. `showFocusBar={false}` is not unique to layer-root zones (cards' inner zones have it too); a tree-depth-conditional contract would be more complex without semantic benefit. Every cross-zone test in the suite asserts a leaf-landing, which is what consumers want regardless of where the destination zone sits in the tree. Universal drill-in is also simpler to reason about — "iter 1 always resolves to a paintable leaf" is a clean rule; "iter 1 sometimes resolves to a zone, sometimes to a leaf, depending on parent depth" is not.
