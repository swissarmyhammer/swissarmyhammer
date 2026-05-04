---
assignees:
- claude-code
depends_on:
- 01KQQSXM2PEYR1WAQ7QXW3B8ME
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffff8580
project: spatial-nav
title: 'Spatial-nav #4: nav.first / nav.last = focus first/last child'
---
## Reference

Part of the spatial-nav redesign. Full design: **`01KQQSXM2PEYR1WAQ7QXW3B8ME`** — read it before starting.

**This component owns:** `nav.first` (Home) and `nav.last` (End). Focus the focused scope's first / last child. On a leaf, no-op.

**Contract (restated from design):**

> First child = the child whose rect is topmost; ties broken by leftmost.
> Last child = the child whose rect is bottommost; ties broken by rightmost.
> Children = registered scopes whose `parent_zone` is the focused scope's FQM.

`nav.first` is identical to `nav.drillIn` (component 2) when the focused scope has children. They share an implementation; the only difference is the key (Enter vs Home) and the React-side editor-focus extension on Enter (which `nav.first` does not get).

## What

### Files to modify

- `swissarmyhammer-focus/src/navigate.rs`:
  - Refactor `edge_command` (line 640). Currently `Direction::First` / `Last` operate on the focused entry's *siblings* (so `Home` in a row of cells = "first cell in that row"). New contract: `First` / `Last` operate on the focused scope's *children* (so `Home` on a focused row container = "first cell in this row"; on a focused leaf = no-op).
  - Concretely: `Direction::First` returns first-child by topmost-then-leftmost ordering; `Direction::Last` returns last-child by bottommost-then-rightmost ordering. Drop the `expect_zone` filter — kind doesn't matter.
  - Decide fate of `Direction::RowStart` / `Direction::RowEnd`. Recommend dropping them (or keeping as aliases for `First` / `Last`) — the user's model has no separate "first in row" concept; the focused scope IS the row. If dropped, sweep callers.

- `swissarmyhammer-focus/src/types.rs`:
  - Update `Direction` enum docstrings for `First` / `Last`. Decide on `RowStart` / `RowEnd` removal.

- `swissarmyhammer-focus/README.md`:
  - Add / update a "## First / Last" section describing the contract. Note the difference from old behaviour if `RowStart` / `RowEnd` were dropped.

### Tests

- **Unit test in `swissarmyhammer-focus/src/navigate.rs::tests` or new `tests/first_last_child.rs`**:
  - Focused leaf -> `nav.first` and `nav.last` return focused FQM (no-op).
  - Focused scope with one child -> both return the child.
  - Focused scope with three children in a row -> `first` returns leftmost, `last` returns rightmost.
  - Focused scope with three children in a column -> `first` returns topmost, `last` returns bottommost.
  - Focused scope with mixed leaves and sub-zones -> both consider all children regardless of kind.
- **Sweep existing tests** that use `Direction::First` / `Last` / `RowStart` / `RowEnd` — update or remove per the new contract. The `unified_trajectories.rs` and `column_header_arrow_nav.rs` tests are the most likely affected.
- Run `cargo test -p swissarmyhammer-focus` and confirm green.

## Acceptance Criteria

- [x] `nav.first` and `nav.last` operate on the focused scope's children, not siblings.
- [x] On a leaf (no children), both return focused FQM (no-op).
- [x] On a container with children, `first` = topmost-then-leftmost, `last` = bottommost-then-rightmost.
- [x] `Direction::RowStart` / `RowEnd` decision documented (kept as aliases or removed; sweep complete either way).
- [x] README "## First / Last" section captures the contract.
- [x] Existing tests pass unchanged or are updated with rationale.
- [x] `cargo test -p swissarmyhammer-focus` passes.

## Implementation Notes

**RowStart / RowEnd decision: kept as aliases for First / Last.**
- The user model has no separate "first in row" concept (the focused scope IS the row), so the variants collapse into the children-of-focused-scope pick.
- Kept (rather than removed) so the wire shape and the TypeScript-side `Direction` union (`kanban-app/ui/src/types/spatial.ts`) and `kanban-app/ui/src/lib/scroll-on-edge.ts` references do not have to migrate. Task #5 is in flight on those files; preserving the variants avoids stepping on #5.
- The pre-redesign vertical-overlap filter is dropped — `RowStart` now resolves to the same child pick as `First`, `RowEnd` to the same as `Last`.

**Tests swept (rewritten in place with rationale comments):**
- `tests/navigate.rs::edge_first_for_leaf_scopes_to_parent_zone` -> `first_on_leaf_returns_focused_self` (leaf has no children -> no-op)
- `tests/navigate.rs::edge_last_for_leaf_scopes_to_parent_zone` -> `last_on_leaf_returns_focused_self`
- `tests/navigate.rs::edge_first_for_zone_scopes_to_sibling_zones` -> `first_on_zone_picks_topmost_leftmost_child` (focused on parent zone, asserts child pick)
- `tests/navigate.rs::edge_row_start_picks_leftmost_in_row_sibling` -> `row_start_alias_picks_leftmost_topmost_child`
- `tests/navigate.rs::edge_row_end_picks_rightmost_in_row_sibling` -> `row_end_alias_picks_rightmost_bottommost_child`
- `tests/navigate.rs::edge_first_at_boundary_returns_focused_self` -> `first_on_topmost_leftmost_leaf_returns_focused_self` (assertion still passes; comment updated)
- `tests/navigate.rs::edge_last_at_boundary_returns_focused_self` -> `last_on_bottommost_rightmost_leaf_returns_focused_self`
- `tests/navigate.rs::edge_row_start_at_boundary_returns_focused_self` -> `row_start_on_leaf_returns_focused_self`

**`unified_trajectories.rs` and `column_header_arrow_nav.rs` did not reference `Direction::First/Last/RowStart/RowEnd`; no edits needed.**

**New unit tests added to `src/navigate.rs::tests`:**
- `first_last_on_leaf_returns_focused_self`
- `first_last_on_zone_with_one_child_returns_that_child`
- `first_last_on_zone_with_row_of_children`
- `first_last_on_zone_with_column_of_children`
- `first_last_considers_children_of_any_kind`
- `row_start_end_are_aliases_for_first_last`
- `first_matches_drill_in_first_child_fallback` (pins the shared-semantics invariant with `drill_in`)

**Verification:** `cargo test -p swissarmyhammer-focus` and full `cargo nextest run` pass; `cargo clippy --all-targets -- -D warnings` clean.

## Workflow

- Use `/tdd`. Write the new first/last child unit tests, refactor `edge_command`, sweep the `Direction` enum and existing tests.
#spatial-nav-redesign

## Review Findings (2026-05-03 19:45)

### Warnings
- [x] `swissarmyhammer-focus/src/navigate.rs:462-479` — `edge_command` reimplements the `parent_zone == focused.fq()` filter inline (`reg.entries_in_layer(layer).filter_map(|s| if s.parent_zone() == Some(&focused_fq_owned) { ... })`) when `SpatialRegistry::child_entries_of_zone(zone_fq)` (registry.rs:1370) already exposes exactly this iterator as a `pub(crate)` helper. The implementer's own pinning test `first_matches_drill_in_first_child_fallback` exists *because* `drill_in` and `edge_command` are two parallel implementations of the same concept — but the right move is to converge them on the existing helper, not pin the duplication with a regression test. Suggested fix: replace the inline `filter_map` with `reg.child_entries_of_zone(focused.fq()).map(|s| (s.fq(), *s.rect()))`. The `entries_in_layer(layer)` filter becomes redundant — children of a focused scope are by construction in the same layer (parent_zone walks never cross layers).
- [x] `swissarmyhammer-focus/src/navigate.rs:494-498` and `swissarmyhammer-focus/src/registry.rs:1517-1528` — The First / cold-start ordering `pixels_cmp(a.top(), b.top()).then(pixels_cmp(a.left(), b.left()))` appears verbatim in two places: `edge_command_from_candidates` (`Direction::First | RowStart` arm) and `SpatialRegistry::drill_in` (cold-start fallback). The task description and module docs both claim "they share an implementation" — they do not; they share semantics via two `min_by` call sites with identical comparators. The pinning test catches behavioral drift after the fact, not before. Suggested fix: extract a `pub(crate) fn first_child_by_top_left<'a>(children: impl Iterator<Item = &'a RegisteredScope>) -> Option<&'a RegisteredScope>` (or a helper on `Rect` itself, e.g. `Rect::top_left_cmp`) and call it from both sites. The pinning test then becomes a test that the two ops *use the same helper*, which is structurally enforced rather than sampled.

### Nits
- [x] `swissarmyhammer-focus/src/navigate.rs:468` — The local binding `let focused_fq_owned = focused.fq().clone();` exists only because the closure inside `filter_map` cannot capture `focused.fq()` by reference across the `entries_in_layer` borrow. If the warning above is addressed (switch to `child_entries_of_zone`), this clone disappears with the inline filter. Otherwise it's a small unnecessary allocation per First/Last keypress.
- [x] `swissarmyhammer-focus/src/types.rs:130-135` and `src/types.rs:167-172` — The `RowStart`/`RowEnd` doc paragraph correctly explains they are aliases, but neither variant carries a `#[deprecated(note = "use Direction::First / Direction::Last")]` attribute. The implementer's rationale (TS side still references them, task #5 in flight) means a `#[deprecated]` attribute now would surface noise on every callsite — this is the right call for now. Recommend filing a follow-up task: "After spatial-nav #5 lands, add `#[deprecated]` to `Direction::RowStart` / `Direction::RowEnd` and migrate any remaining callsites to `First` / `Last`." Soft prose deprecation in the docstring is the bridge until the formal attribute can land.

## Resolution (2026-05-04)

**Warning 1 + Nit 1 (eliminated together):** `edge_command` now calls `reg.child_entries_of_zone(focused.fq())` directly. The inline `entries_in_layer + filter_map` is gone; the `focused_fq_owned` clone disappeared with it. The defunct `edge_command_from_candidates` private helper was deleted — it only existed as a thin wrapper around the inlined comparators.

**Warning 2 (extracted shared helper):** Added two `pub(crate)` free functions in `swissarmyhammer-focus/src/registry.rs`:
- `first_child_by_top_left<'a>(children: impl Iterator<Item = &'a RegisteredScope>) -> Option<&'a RegisteredScope>`
- `last_child_by_bottom_right<'a>(children: impl Iterator<Item = &'a RegisteredScope>) -> Option<&'a RegisteredScope>`

Both `SpatialRegistry::drill_in`'s cold-start fallback and `navigate::edge_command`'s `Direction::First / Last / RowStart / RowEnd` arms now call these helpers. The duplicate `min_by` / `max_by` comparators are gone; divergence between drill-in and `nav.first` is structurally impossible. The `first_matches_drill_in_first_child_fallback` test stays as a behavioural backstop (still passes); its comment now explains that the structural sharing is what guarantees the invariant.

**Nit 2 (follow-up filed):** Created task `01KQR7N8E5ZAPY4KK4MN8NXY77` — "Spatial-nav: mark Direction::RowStart / RowEnd #[deprecated] after #5 lands" — depending on the design root so it surfaces once #5 is done.