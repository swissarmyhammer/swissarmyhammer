---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff8480
project: spatial-nav
title: 'Spatial nav: beam test starves cross-region moves — score all directional candidates, not just in-beam'
---
## What

From the LeftNav (narrow left strip, near the top of the window), pressing Right doesn't reach the first visible grid row selector or board column. Same failure mode in other "cross-region" moves where the source rect doesn't vertically overlap any candidate-to-the-right that's "in the beam."

### Root cause

`swissarmyhammer-spatial-nav/src/spatial_nav.rs:118-128` — `is_in_beam`:

```rust
fn is_in_beam(source: &Rect, candidate: &Rect, direction: Direction) -> bool {
    match direction {
        Direction::Right | Direction::Left => {
            candidate.y < source.bottom() && candidate.bottom() > source.y
        }
        // ...
    }
}
```

For Left/Right, this requires **any** vertical overlap (>=1px) between source and candidate. If the LeftNav button is at `y:100-140` and the first grid row selector is at `y:200-236`, there's zero overlap — the row selector is **not** "in-beam."

`find_cardinal` (`spatial_nav.rs:194-224`) then pools candidates:

```rust
// simplified
let pool = if in_beam.is_empty() { &out_beam } else { &in_beam };
pool.iter().min_by_key(score)
```

The out-of-beam fallback **only fires when `in_beam.is_empty()`**. In practice, there's almost always at least one in-beam candidate to the right of the LeftNav (e.g. any toolbar button, header cell, or scope that happens to vertically overlap), so `in_beam` is non-empty, `out_beam` is silently discarded, the row selector (in `out_beam`) is never scored, and Right from LeftNav lands on a random small target instead of the main content.

Same culling exists in the JS shim at `kanban-app/ui/src/test/spatial-shim.ts:136-148`, so the parity fence holds — both implementations are wrong together.

### Fix

Adopt the Android FocusFinder "beam beats" arbitration rule instead of a hard pool-split cutoff:

1. Score the best candidate within each pool (in-beam, out-of-beam).
2. If both exist, the in-beam winner is preferred **unless** the out-of-beam winner is dramatically closer — specifically, unless its far edge is nearer than the in-beam winner's near edge along the travel direction.

This preserves in-beam preference for normal same-row/same-column moves (grid cell right lands on the next cell), while unblocking cross-region moves when the nearest in-beam hit is absurdly far (LeftNav right lands on the first visible content scope instead of a far-right toolbar pill).

### Files modified

- `swissarmyhammer-spatial-nav/src/spatial_nav.rs` — `find_cardinal` now scores both pools and arbitrates via `in_beam_dominates` (major-to-near vs major-to-far-edge comparison).
- `kanban-app/ui/src/test/spatial-shim.ts` — mirrored the same arbitration in the JS shim.
- `swissarmyhammer-spatial-nav/src/spatial_nav.rs` tests + `kanban-app/ui/src/test/spatial-parity-cases.json` — four new scenarios: `right_from_leftnav_reaches_grid_row_selector_when_no_in_beam_candidate_closer`, `right_from_leftnav_prefers_in_beam_candidate_when_available`, `left_from_grid_row_selector_reaches_leftnav`, `down_from_perspective_bar_reaches_grid_header_when_no_horizontal_overlap`.

### Out of scope

- Changing the scoring formula itself (13:1 weighting). That's the Android constant; only touch it if the new arbitration surfaces a clear case where it's wrong.
- Introducing direction-dependent "major axis dominance" variants. Keep the algorithm a single formula.

## Acceptance Criteria

- [x] From a LeftNav view-switcher button, pressing Right lands on the nearest-visible scope in the main content area — unit test + parity test + existing `spatial-nav-leftnav.test.tsx` `l` test all green
- [x] From the first row selector of a grid, pressing Left lands on a LeftNav button (symmetry check) — unit test + parity test
- [x] From the perspective tab bar, pressing Down lands on the nearest content scope below — unit test + parity test
- [x] In-beam candidates are still preferred when available — `beam_candidate_preferred_over_closer_out_of_beam` + `right_from_leftnav_prefers_in_beam_candidate_when_available` green
- [x] No regression in any existing spatial-nav test (Rust or JS) — all 66 Rust unit tests + 24 parity cases + 61 browser spatial-nav tests green
- [x] Parity between Rust and JS shim maintained — 4 new parity cases green on both sides

## Tests

- [x] Added Rust unit tests in `swissarmyhammer-spatial-nav/src/spatial_nav.rs`:
  - `right_from_leftnav_reaches_grid_row_selector_when_no_in_beam_candidate_closer`
  - `right_from_leftnav_prefers_in_beam_candidate_when_available`
  - `left_from_grid_row_selector_reaches_leftnav`
  - `down_from_perspective_bar_reaches_grid_header_when_no_horizontal_overlap`
- [x] Added the same 4 cases to `kanban-app/ui/src/test/spatial-parity-cases.json` — pass in both `tests/parity.rs` and `spatial-shim-parity.test.ts`
- [x] `spatial-nav-leftnav.test.tsx` already covered the `l` (Right) from LeftNav to main body scope; that test passes against the new algorithm
- [x] `cargo test -p swissarmyhammer-spatial-nav` — 66 unit tests + 1 parity test all green
- [x] `cd kanban-app/ui && npm test` — 1356 tests green (the pre-existing `nav-bar.test.tsx` module-resolution failure is unrelated; touched neither that file nor any file it imports)

## Workflow

- Used TDD — wrote the 4 failing Rust unit tests first, watched 3 fail (4th was a regression guard), implemented the fix, then mirrored in the shim + parity JSON.
- The algorithm lives in Rust; the JS shim is a faithful mirror verified by the shared parity fixture.

