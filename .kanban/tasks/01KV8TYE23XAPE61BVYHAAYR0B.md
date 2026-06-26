---
comments:
- actor: claude-code
  id: 01kw31hmvjq8my4z06m7hq4dq3
  text: |-
    Picked up. Reproduced the two clippy errors with `cargo clippy -p swissarmyhammer-focus --all-targets -- -D warnings` (clippy 1.95.0):
    1. navigate.rs — `doc_lazy_continuation`: the `3b.` list item (not a valid markdown ordered marker) was an unindented lazy continuation of item 3.
    2. state.rs `focus_lost` — `too_many_arguments` (8/7).

    Fixes (root-cause, no #[allow]):
    - navigate.rs: indented the `3b.` doc line by 3 spaces so it aligns as a proper continuation of item 3's content (clippy's own suggestion).
    - state.rs: introduced a `LostScope<'a>` param struct (fq, parent_zone, layer_fq, rect) bundling the lost-scope wire descriptor. `focus_lost` now takes `(registry, snapshot, lost: LostScope, window)` — 5 args incl self. Re-exported `LostScope` from lib.rs. Updated the one production caller (server.rs `handle_focus_lost`) and all 7 test call sites (focus_lost.rs ×4, spatial_nav_soak.rs ×3).

    Verified GREEN:
    - clippy: exit 0, zero warnings.
    - `cargo nextest run -p swissarmyhammer-focus`: 131/131 passed.
    - `cargo fmt`: clean, no churn.
  timestamp: 2026-06-26T22:41:58.130528+00:00
- actor: claude-code
  id: 01kw31mxz9ssxkmkctwdvp63qf
  text: 'Adversarial double-check: PASS. Confirmed no #[allow] suppressions, exact 1:1 LostScope field mapping at all 8 call sites (no value swaps), behavior-preserving body, correct doc fix, no scope creep. Work done and green; leaving in `doing` for /review.'
  timestamp: 2026-06-26T22:43:45.769632+00:00
position_column: doing
position_ordinal: '8180'
title: Pre-existing clippy errors in swissarmyhammer-focus block `clippy -D warnings`
---
## What

`cargo clippy -p swissarmyhammer-focus --all-targets -- -D warnings` fails with two errors (clippy 1.95.0). Pre-existing — surfaced while running clippy across the dependency graph for an unrelated task (tags⇄body sync, `^8q2v2vf`); `swissarmyhammer-focus` was not modified by that work.

### Errors

1. `crates/swissarmyhammer-focus/src/state.rs:319` — `error: this function has too many arguments (8/7)` on `pub fn focus_lost(&mut self, registry, snapshot, ..., window: Option<WindowLabel>) -> Option<FocusChangedEvent>`. Fix: bundle related params into a struct (e.g. a `FocusLostCtx`/params struct) rather than `#[allow(clippy::too_many_arguments)]`.
2. `crates/swissarmyhammer-focus/src/navigate.rs:45` — `error: doc list item without indentation`. Fix: re-indent the continuation line of the doc-comment list item.

## Acceptance Criteria
- [ ] `cargo clippy -p swissarmyhammer-focus --all-targets -- -D warnings` passes with zero errors/warnings.
- [ ] No `#[allow(...)]` suppressions added; fixes address the root cause (param struct + doc indentation).

## Tests
- [ ] `cargo clippy -p swissarmyhammer-focus --all-targets -- -D warnings` → clean.
- [ ] `cargo test -p swissarmyhammer-focus` → green (the `focus_lost` refactor must not change behavior; update callers).

## Notes
- Discovered during `^8q2v2vf`. Independent of that change. #test-failure