---
assignees:
- claude-code
depends_on:
- 01KQZ7VR7JK1QD5QJDDKB529JG
- 01KQZ7W84TTCCEBZYPRKBVSJ3F
- 01KQZ7WR2SYN4W9DSF9JKH6FQ3
position_column: todo
position_ordinal: f380
project: spatial-nav
title: 'swissarmyhammer-focus README: keep design + algorithm in sync with the post-rebuild kernel'
---
## What

Audit and update `swissarmyhammer-focus/README.md` so it accurately describes the post-rebuild kernel. The README is the public-facing design doc for the crate (`Cargo.toml:readme = "README.md"`) and must stay succinct — current length 79 lines, target ≤ 120 lines after the update.

The README today (commit `a9e7e746c`) describes the **pre-rebuild** kernel: state-bearing `SpatialRegistry`, hard in-band filter on cardinal nav, no mention of `NavSnapshot` / `FocusOp` / `decide()`. After the cards in this project land — motions fix (`01KQZ7VR7JK1QD5QJDDKB529JG`), stateless contract (`01KQZ7W84TTCCEBZYPRKBVSJ3F`), `decide()` implementation (`01KQZ7WR2SYN4W9DSF9JKH6FQ3`), and the cutover sequence — the kernel shape changes substantively.

This task is the documentation backstop: run after the rebuild and the per-op motion-validation suites pass, then rewrite the README to match.

### Specific drift to fix

- [ ] **Cardinal nav: in-band is a score bias, not a hard filter** — line 27-28 currently states "overlap the focused rect on the cross axis" as a candidate-eligibility rule. Per `01KQZ7VR7JK1QD5QJDDKB529JG`'s fix, in-band is a *bias*: out-of-band candidates remain reachable when no in-band target exists (matches the Android beam-search shape). Rewrite the bullet so the algorithm reads: "(1) lie strictly in the half-plane of D, (2) are not the focused scope; in-band candidates score lower (better) — out-of-band candidates are still reachable when no in-band target exists."
- [ ] **First / Last: parent's children, not focused's children** — current line 49-52 says "child of the focused scope's parent." Verify this still matches `BeamNavStrategy::next` for `Direction::First` / `Direction::Last` after the rebuild. Add the layer-root fallback explicitly: "focused scope at the layer root → children-of-self." Today's text says "stay-put" at the layer root, which is wrong post-rebuild.
- [ ] **Stateless surface** — add a new section "## Stateless decision API" describing `decide(state, op, snapshot, window) -> (FocusState, Option<FocusChangedEvent>)` and the `NavSnapshot` / `FocusOp` / `FocusState` shapes (defined by card `01KQZ7W84TTCCEBZYPRKBVSJ3F`). One short paragraph plus a 6-row table mapping `FocusOp` variants to behaviour. Reference: `swissarmyhammer-focus/src/stateless/decide.rs`.
- [ ] **Drill in: cold/warm cascade explicitly** — current "drill down" section is correct in spirit but doesn't name `last_focused_by_fq` (the actual field). Update to say "(1) prefer `state.last_focused_by_fq.get(focused)` when warm; (2) fall back to the topmost-then-leftmost child of the focused scope; (3) leaf with no children → stay-put."
- [ ] **First ≡ drill-in cold-start contract** — add a one-sentence note documenting that `Direction::First` and the cold-start branch of drill-in share the topmost-then-leftmost helper (`first_child_by_top_left`) so the two ops produce the same target on a parent zone with no `last_focused`. The kernel test `first_matches_drill_in_first_child_fallback` is the structural backstop on this; the README should mention the contract, not the test.
- [ ] **Tie-break wording** — current line 32 says "leaves win over scopes-with-children." Verify the actual tie-break in `score_candidate` / `pick_best_candidate` post-rebuild and rewrite if the predicate has shifted (e.g., parent_zone vs has-children).
- [ ] **Scrolling section** — verify retry depth is still capped at 1 post-rebuild (`scroll-on-edge.ts`); update if the consumer-side retry shape changed.
- [ ] **No new sections beyond what's listed above.** The README is an executive summary, not a manual — keep it ≤ 120 lines after the update.

### What NOT to change

- The "Headless spatial-navigation kernel" intro (lines 1-5) — accurate.
- "Boundary / Layer" section (lines 13-17) — accurate.
- "Overrides (rule 0)" section (lines 54-58) — accurate post-rebuild per the override-first contract.
- "No-silent-dropout" section (lines 60-64) — central invariant; do not rewrite.
- "Coordinate system" section (lines 66-71) — accurate.

### Verification

After the rewrite, walk the README top-to-bottom and confirm each algorithmic claim has a matching code reference:
- Cardinal: `swissarmyhammer-focus/src/navigate.rs::geometric_pick` / `score_candidate`
- First / Last: `BeamNavStrategy::next` `Direction::First` / `Direction::Last` arms
- Drill in / out: `swissarmyhammer-focus/src/registry.rs::drill_in` / `drill_out`
- Stateless: `swissarmyhammer-focus/src/stateless/decide.rs`
- Override: scope-level `overrides` map walk in `BeamNavStrategy::next`

Each numbered acceptance bullet maps to one code site — if no code site implements the claim, the README is fiction and must be cut.

## Acceptance Criteria

- [ ] `swissarmyhammer-focus/README.md` ≤ 120 lines.
- [ ] Every operation section in the README maps 1:1 to a code site listed in "Verification" above.
- [ ] No mention of "hard in-band filter" or any phrasing that implies out-of-band candidates are dropped from cardinal nav.
- [ ] First / Last layer-root fallback is documented (children-of-self when parent is None).
- [ ] A "Stateless decision API" section exists, names `decide()`, and lists each `FocusOp` variant.
- [ ] Drill-in mentions `last_focused_by_fq` by name and the topmost-leftmost cold-start fallback.
- [ ] First ≡ drill-in cold-start contract is documented.
- [ ] `Cargo.toml:readme = "README.md"` still resolves; `cargo doc -p swissarmyhammer-focus` builds without doctest failures introduced by the rewrite.

## Tests

- [ ] `cargo doc -p swissarmyhammer-focus --no-deps` builds clean (no broken intra-doc links if the README is included via `#![doc = include_str!("../README.md")]` — verify whether `lib.rs` does this; if so, doctests in the README must compile).
- [ ] New Rust integration test `swissarmyhammer-focus/tests/readme_claims.rs` (lightweight): assert each named code site exists by `use`-importing the symbol the README references — `swissarmyhammer_focus::stateless::decide`, `swissarmyhammer_focus::registry::SpatialRegistry::drill_in`, `swissarmyhammer_focus::registry::SpatialRegistry::drill_out`, `swissarmyhammer_focus::navigate::BeamNavStrategy`. The test compiles ⇔ each symbol the README mentions is real. Symbol-rename refactors will fail this test, prompting a README update.
- [ ] Test command: `cargo nextest run -p swissarmyhammer-focus readme_claims` — passes.
- [ ] `wc -l swissarmyhammer-focus/README.md` ≤ 120.

## Workflow

- Use `/tdd` — write `tests/readme_claims.rs` first against the symbols the rewritten README will mention. Then rewrite the README. Then run `wc -l` and `cargo doc` to verify the size + doc-build constraints.

#stateless-rebuild #docs