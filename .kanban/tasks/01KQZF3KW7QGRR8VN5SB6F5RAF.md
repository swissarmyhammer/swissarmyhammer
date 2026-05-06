---
assignees:
- claude-code
depends_on:
- 01KQZ7VR7JK1QD5QJDDKB529JG
- 01KQZ7W84TTCCEBZYPRKBVSJ3F
- 01KQZ7WR2SYN4W9DSF9JKH6FQ3
- 01KQZJKQDWY3ZFMVW9GEH2VQ4C
- 01KQZJP727DMDTX6RH7DMCWAPN
position_column: todo
position_ordinal: f380
project: spatial-nav
title: 'swissarmyhammer-focus README: keep design + algorithm in sync with the post-rebuild kernel'
---
## What

Audit and update `swissarmyhammer-focus/README.md` so it accurately describes the post-rebuild kernel. The README is the public-facing design doc for the crate (`Cargo.toml:readme = "README.md"`) and must stay succinct — current length 79 lines, target ≤ 120 lines after the update.

The README on `main` describes the **pre-rebuild** kernel: state-bearing `SpatialRegistry`, hard in-band filter on cardinal nav, no mention of `NavSnapshot` / `FocusOp` / `decide()`, references to `BeamNavStrategy` and `navigate.rs`. After cards 1–5 of this project land:

- card 1 (`01KQZ7VR7JK1QD5QJDDKB529JG`) — motions fix
- card 2 (`01KQZ7W84TTCCEBZYPRKBVSJ3F`) — types lock
- card 3 (`01KQZ7WR2SYN4W9DSF9JKH6FQ3`) — `decide()` body
- card 4 (`01KQZJKQDWY3ZFMVW9GEH2VQ4C`) — React migration to `spatial_decide`
- card 5 (`01KQZJP727DMDTX6RH7DMCWAPN`) — delete legacy (`BeamNavStrategy`, `SpatialState`, per-op IPCs, per-scope `last_focused`, `navigate.rs`)

…the kernel shape changes substantively: `BeamNavStrategy` and `navigate.rs` are gone, `SpatialState` is gone, `FocusScope::last_focused` is gone, all algorithms live in `swissarmyhammer-focus/src/stateless/decide.rs`, all consumer dispatch is one Tauri command (`spatial_decide`).

This task is the documentation backstop: it runs **after card 5 deletes the legacy surface**, then rewrites the README against the surviving symbols only.

### Specific drift to fix

- [ ] **Cardinal nav: in-band is a score bias, not a hard filter** — current line 27-28 phrasing reads as a candidate-eligibility rule. Per card 1's fix, in-band is a *bias*: out-of-band candidates remain reachable when no in-band target exists (Android beam-search shape). Rewrite the bullet to: "(1) lie strictly in the half-plane of D, (2) are not the focused scope; in-band candidates score lower (better) — out-of-band candidates are still reachable when no in-band target exists."
- [ ] **First / Last: parent's children, not focused's children** — current line 49-52 says "child of the focused scope's parent." Verify against `decide()`'s `EdgeFirst` / `EdgeLast` arms in `stateless/decide.rs`. Add the layer-root fallback explicitly: "focused scope at the layer root → children-of-self." Today's text says "stay-put" at the layer root, which is wrong post-rebuild.
- [ ] **Single-entrypoint surface** — replace any reference to `BeamNavStrategy` / per-op functions with the single `decide(state, op, snapshot, window) -> (FocusState, Option<FocusChangedEvent>)` entry point. Add a "## Decision API" section: one short paragraph plus a table mapping each `FocusOp` variant (`Cardinal`, `EdgeFirst`, `EdgeLast`, `DrillIn`, `DrillOut`, `Click`, `FocusLost`, `ClearFocus`, `PushLayer`, `PopLayer`) to its behaviour. Reference: `swissarmyhammer-focus/src/stateless/decide.rs`.
- [ ] **NavSnapshot is the input shape** — explain that consumers ship a `NavSnapshot { layer_fq, scopes }` per call (built React-side from `LayerScopeRegistry`), not a long-lived registry. Two sentences.
- [ ] **Drill in: cold/warm cascade explicitly** — current "drill down" section is correct in spirit but doesn't name `last_focused_by_fq` (the actual field on `FocusState`). Update to: "(1) prefer `state.last_focused_by_fq.get(focused)` when warm; (2) fall back to the topmost-then-leftmost child of the focused scope; (3) leaf with no children → stay-put."
- [ ] **First ≡ drill-in cold-start contract** — add a one-sentence note that `EdgeFirst` and the cold-start branch of `DrillIn` share the topmost-then-leftmost helper, so on a parent zone with no `last_focused` the two ops produce the same target.
- [ ] **Tie-break wording** — current "leaves win over scopes-with-children" line: verify against the actual tie-break inside `decide()` post-rebuild and rewrite if the predicate shifted.
- [ ] **Scrolling section** — verify retry depth is still capped at 1 post-rebuild (`kanban-app/ui/src/lib/scroll-on-edge.ts`); update if the consumer-side retry shape changed.
- [ ] **No new sections beyond what's listed above.** The README is an executive summary, not a manual — keep it ≤ 120 lines after the update.

### What NOT to change

- The "Headless spatial-navigation kernel" intro (lines 1-5) — accurate.
- "Boundary / Layer" section (lines 13-17) — accurate.
- "Overrides (rule 0)" section (lines 54-58) — `decide()` runs the override map first per the same contract.
- "No-silent-dropout" section (lines 60-64) — central invariant; do not rewrite.
- "Coordinate system" section (lines 66-71) — accurate.

### Verification

After the rewrite, walk the README top-to-bottom and confirm each algorithmic claim has a matching code reference. The reference list **after card 5**:

| README claim | Code site |
|---|---|
| Cardinal beam pick | `stateless::decide::cardinal_pick` (or whatever helper inside `decide.rs` owns the score) |
| First / Last | `stateless::decide` `FocusOp::EdgeFirst` / `EdgeLast` arms |
| Drill in / out | `stateless::decide` `FocusOp::DrillIn` / `DrillOut` arms |
| Decision entry point | `swissarmyhammer_focus::stateless::decide` |
| Override (rule 0) | scope-level `overrides` map walk inside `decide()` |
| Snapshot input | `swissarmyhammer_focus::stateless::types::NavSnapshot` |
| State output | `swissarmyhammer_focus::stateless::types::FocusState` |

If no surviving code site implements a claim, the README is fiction and the claim must be cut.

## Acceptance Criteria

- [ ] `swissarmyhammer-focus/README.md` ≤ 120 lines.
- [ ] Every operation section in the README maps 1:1 to a code site listed in "Verification" above (all of them under `stateless::*` — no references to `BeamNavStrategy`, `NavStrategy`, `SpatialState`, `navigate.rs`, or `SpatialRegistry::record_focus`).
- [ ] No mention of "hard in-band filter" or any phrasing that implies out-of-band candidates are dropped from cardinal nav.
- [ ] First / Last layer-root fallback is documented (children-of-self when parent is None).
- [ ] A "## Decision API" section exists, names `decide()`, names `NavSnapshot` / `FocusState` / `FocusOp`, and lists every `FocusOp` variant in a table.
- [ ] `DrillIn` mentions `last_focused_by_fq` by name and the topmost-leftmost cold-start fallback.
- [ ] First ≡ drill-in cold-start contract is documented.
- [ ] `Cargo.toml:readme = "README.md"` still resolves; `cargo doc -p swissarmyhammer-focus --no-deps` builds clean.

## Tests

- [ ] `cargo doc -p swissarmyhammer-focus --no-deps` builds clean (no broken intra-doc links if the README is included via `#![doc = include_str!("../README.md")]`; if `lib.rs` does this, README doctests must compile).
- [ ] New Rust integration test `swissarmyhammer-focus/tests/readme_claims.rs`: a `use` block listing **only** the symbols the rewritten README mentions, all under `swissarmyhammer_focus::stateless::*`:
  ```rust
  use swissarmyhammer_focus::stateless::{
      decide,
      types::{FocusOp, FocusState, FocusDecision, NavSnapshot, SnapshotScope},
  };
  ```
  The test compiles ⇔ every symbol the README cites still exists. Any future rename will fail this test and force a README update. **No imports from `navigate`, `state`, or `BeamNavStrategy`** — those are deleted by card 5.
- [ ] Test command: `cargo nextest run -p swissarmyhammer-focus readme_claims` — passes.
- [ ] `wc -l swissarmyhammer-focus/README.md` ≤ 120.

## Workflow

- Use `/tdd` — write `tests/readme_claims.rs` first against the surviving stateless symbols. Then rewrite the README. Then run `wc -l` and `cargo doc` to verify the size + doc-build constraints.

#stateless-rebuild #docs