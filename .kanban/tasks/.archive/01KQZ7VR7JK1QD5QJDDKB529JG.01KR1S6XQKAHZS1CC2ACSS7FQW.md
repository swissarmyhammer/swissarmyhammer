---
assignees:
- claude-code
position_column: todo
position_ordinal: e580
project: spatial-nav
title: 'motions: fix cardinal beam + first/last + drill semantics on the current kernel'
---
## Why this is card 1

User-visible motion bugs that don't require the architectural rebuild. Fix on the current (pre-cutover) kernel; the rebuild keeps these algorithms intact.

## Bugs to fix

### 1. Cardinal nav requires "perfect alignment"

`swissarmyhammer-focus/src/navigate.rs::geometric_pick` (and `score_candidate` / `pick_best_candidate`) drops every candidate that doesn't strictly overlap the focused scope's perpendicular band. For Right, `vertical_overlap(from, cand)` must be true or the candidate is skipped — there's no widening fallback. Result: from a thin nav-rail row, pressing Right does nothing because nothing in the board area shares that 28px Y band.

**Fix:** drop the hard `if !in_beam { continue; }` filter. Keep `in_beam` as a score *bias* — in-band candidates score lower (better) by some factor, out-of-band candidates are still reachable when no in-band target exists. Match the Android beam-search shape that the rest of the algorithm references.

Lines: `navigate.rs:355-357`, `navigate.rs:584-587`, `navigate.rs:611-613`.

### 2. First / Last from a focused leaf stays put

`edge_command` walks `children_of(focused.fq)`. A leaf has no children → empty set → stay-put. Vim G / gg semantics want first/last *sibling*, i.e. `children_of(focused.parent_zone)`.

**Fix already in HEAD** (`navigate.rs:471-493` after `d0460d061`). Verify and add a parametrised regression test covering: leaf with siblings, layer-root scope (parent_zone == None) falling back to children-of-self.

### 3. Drill in / drill out

`SpatialRegistry::drill_in` should: prefer `last_focused_by_fq.get(focused)`; fall back to `first_child_by_top_left(children_of(focused))` (drill into first child by reading order). `SpatialRegistry::drill_out` returns `registry.scopes[focused].parent_zone`.

Verify these are correct now and add tests if missing.

## Tests

- New `navigate.rs` test cases for cardinal nav with vertical-misaligned candidates (proves the bug fix).
- Vim G / gg from a focused leaf with siblings (regression for #2).
- Drill in cold-start (no last_focused) → first child; warm-start (last_focused set) → that child.
- Drill out from a leaf → parent_zone; from a top-level scope → focused (stay-put).

## Acceptance

- `cargo test -p swissarmyhammer-focus` green
- `pnpm -C kanban-app/ui test` green (no UI test changes expected)
- Manual: in the running app, Right from any nav-rail leaf lands on the nearest scope in the board area (not stay-put).

## Files

- `swissarmyhammer-focus/src/navigate.rs`
- `swissarmyhammer-focus/src/registry.rs` (drill_in / drill_out only — no API surface change)
#stateless-rebuild