---
assignees:
- claude-code
depends_on:
- 01KQQSXM2PEYR1WAQ7QXW3B8ME
position_column: todo
position_ordinal: cf80
project: spatial-nav
title: 'Spatial-nav #1: geometric cardinal pick replaces structural cascade'
---
## Reference

Part of the spatial-nav redesign. Full design: **`01KQQSXM2PEYR1WAQ7QXW3B8ME`** — read it before starting, especially the "Contract" and "Invariants" sections.

**This component owns:** `nav.{up,down,left,right}` — the four cardinal direction operations. Replace the iter-0 / iter-1 / drill-out cascade in `swissarmyhammer-focus/src/navigate.rs::cardinal_cascade` with a single layer-wide geometric search.

**Contract (restated from design):**

> Cardinal nav from `focused` in direction D returns the registered scope (leaf or zone, in the same `layer_fq`) whose rect minimises the Android beam score (`13 * major² + minor²`) across ALL registered scopes in the layer that pass the in-beam test for D and lie strictly in the half-plane of D. No structural filtering — `parent_zone` and `is_zone` are tie-breakers and observability only.

When the half-plane is empty (focused at the visual edge of the layer), return `focused_fq` (stay-put, per the no-silent-dropout invariant).

## What

### Files to modify

- `swissarmyhammer-focus/src/navigate.rs`:
  - Replace `cardinal_cascade` (line 320) with `geometric_pick(reg, focused, focused_fq, direction)`. Iterates `reg.entries_in_layer(focused.layer_fq())`, filters out the focused entry itself, scores via existing `score_candidate` (line 713), returns lowest-scoring candidate's FQM. Empty result set → return `focused_fq`.
  - Tie-break order: prefer leaves over zones (so when geometric pick lands equally on a `showFocusBar=false` container and an inner leaf, the user sees the indicator paint).
  - Delete `beam_among_in_zone_any_kind`, `beam_among_siblings`, `parent_zone_resolution`, `ParentResolution`, `drill_into_natural_leaf`. The leaf-tie-break absorbs `drill_into_natural_leaf`'s purpose for the common case.
  - Keep `score_candidate`, `pick_best_candidate`, the in-beam helpers, the override path, and the edge-command path unchanged.
  - Update the module-level docstring to describe the geometric algorithm.

- `swissarmyhammer-focus/README.md`:
  - **Rewrite the "## The cascade" section** — replace the three-step cascade prose with the keyboard-as-mouse geometric model. Cross-reference `tests/cross_zone_geometric_nav.rs`.
  - Keep the "## Overrides" section (rule 0 unchanged).
  - Update "## No-silent-dropout" to mention "stay-put when half-plane is empty."
  - Strengthen "## Kind is not a filter" — the geometric algorithm is even more committed to this principle.
  - **Note:** components #2, #3, #4, #5, #6 each own their own README sections — do not pre-empt them; they will land in their own PRs.

- `swissarmyhammer-focus/tests/fixtures/mod.rs`:
  - Extend `RealisticApp` to include a `ui:left-nav` zone (vertical sidebar to the left of `ui:perspective-bar`) with at least two `view:{id}` leaves inside, AND `column:{id}` zones inside `ui:board` with cards. Geometry must mirror production (left-nav is tall, perspective-bar is thin, columns sit inside the board area).
  - Expose `left_nav_fq()`, `left_nav_view_button_grid_fq()`, `column_todo_fq()`, etc.

### Tests

- **New unit tests in `swissarmyhammer-focus/src/navigate.rs::tests`**: lonely scope returns focused FQM; one neighbor in direction wins; two neighbors at different distances — closer wins; tied distances — leaf wins over zone; cross-`parent_zone` candidate wins when geometrically nearer than in-zone candidate.
- **New integration test `swissarmyhammer-focus/tests/cross_zone_geometric_nav.rs`**: assert each of the four reported cross-zone bugs returns the visibly-adjacent target. One test per direction × starting-FQM combination:
  - Left from leftmost `perspective_tab` lands inside `ui:left-nav`.
  - Up from `column:{id}` lands inside `ui:perspective-bar`.
  - Down from a `perspective_tab` lands inside the perspective body (a column, etc.).
  - Up from a column lands on perspective bar; Up from perspective bar lands on navbar.
- **Regression sweep**: re-run every test in `swissarmyhammer-focus/tests/` (`card_directional_nav`, `column_header_arrow_nav`, `navbar_arrow_nav`, `perspective_bar_arrow_nav`, `unified_trajectories`, `in_zone_any_kind_first`, `overrides`, `no_silent_none`). For any failure, write a short note in the PR description: is the new behaviour correct (test should be updated) or wrong (algorithm needs work)? Most should pass unchanged because in-card and in-row geometric answers match structural answers.
- Run `cargo test -p swissarmyhammer-focus` and confirm green.

## Acceptance Criteria

- [ ] `BeamNavStrategy::next` for cardinal directions runs `geometric_pick`; the iter-0 / iter-1 / drill-out cascade is gone.
- [ ] All four cross-zone regression tests in `cross_zone_geometric_nav.rs` pass.
- [ ] Existing tests pass unchanged OR are updated with documented rationale.
- [ ] No-silent-dropout: `next` always returns a `FullyQualifiedMoniker`, never `None`, never `target=None` in the IPC, never collapses to engine root.
- [ ] Layer boundary respected: no candidate from a different `layer_fq` is considered.
- [ ] `navOverride` walls / redirects still run as rule 0.
- [ ] README "## The cascade" section rewritten to describe geometric model.
- [ ] `cargo test -p swissarmyhammer-focus` passes.

## Workflow

- Use `/tdd`. Write the four cross-zone regressions in `cross_zone_geometric_nav.rs` first (RED against current code), implement `geometric_pick`, watch them go green, then sweep the existing test suite and resolve each delta.
#spatial-nav-redesign