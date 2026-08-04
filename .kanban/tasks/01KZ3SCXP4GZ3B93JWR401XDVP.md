---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz5zggtd63hy8xnsg5a4ny3x
  text: |-
    ### implement — changed

    Re-read the current file first per the task's own warning — the walker family had already been generalized by ^xjyb2qf into `walk`/`walk_children`/`walk_conditional`/`walk_alternative`/`walk_boolean`/`boolean_chain`, plus `collect_functions`'s own separate whole-tree recursion. None of these had a depth cap.

    **Approach chosen: option 1, explicit recursion-depth cap (not the iterative rewrite).** Justification: the walk family threads several pieces of local state through mutual recursion (nesting, loop_nesting, else-if chain position, boolean sequence/operand counters). A correctness-preserving iterative rewrite of all six mutually-recursive functions plus `collect_functions` was high-risk for this change's actual goal (stack safety) versus a small, uniform depth parameter threaded exactly the way `nesting`/`loop_nesting` already are. Chose the lower-risk option explicitly sanctioned by the task text ("whichever is simpler to get right").

    **Implementation**:
    - Added `MAX_TRAVERSAL_DEPTH: u32 = 256` (two orders of magnitude above the deepest pinned fixture, depth 4, and above any plausible real function).
    - Threaded a `depth: u32` parameter (raw tree depth, distinct from the semantic `nesting`/`loop_nesting` counters) through `walk`, `walk_children`, `walk_conditional`, `walk_alternative`, `walk_boolean`, `boolean_chain`, and separately through `collect_functions`'s own tree walk. Each function that can be re-entered directly (not only via `walk`'s own guard) — `walk_conditional`, `walk_alternative`, `boolean_chain` — re-checks the cap itself.
    - Added `Tally.is_partial` / `Tally::depth_capped()` and a new public `FunctionComplexity.is_partial: bool` field. When any walker hits the cap while scoring a function, that function's `is_partial` is set true — this repo's "not computed, never a silent wrong number" convention (from ^xjyb2qf), applied per-function rather than per-language.
    - `FunctionComplexity::exceeds_gates()` now returns `true` unconditionally when `is_partial`, so a partial function is never silently read as "under the gates" — this matters because the only consumer (`swissarmyhammer-validators::review::probes::run_complexity`) treats an empty `functions` result as the positive fact "every function is under both gates"; omitting a partial function from `functions` would have been exactly the silent-wrong-number failure mode the task warns against.
    - Updated `probes.rs::complexity_row` to emit a distinct detail message for `is_partial` rows ("cognitive complexity NOT FULLY COMPUTED ... numbers below are a lower bound") instead of the normal evidence line.

    **Tests** (in `complexity.rs`):
    - `pathological_nesting_does_not_crash_and_is_reported_as_partial` — 5,000 levels of nested `if true { }` inside one function. Runs in 0.031s, does not crash, function is found (`functions.len() == 1`), `is_partial == true`, `exceeds_gates() == true`.
    - `nesting_well_under_the_traversal_cap_is_never_marked_partial` — 32 levels (far above every other fixture, far below the 256 cap): `is_partial == false`, `max_nesting_depth == 32` exactly, proving the cap doesn't clip real-but-unusual depth.
    - `pinned_small_fixtures_are_unaffected_by_the_traversal_cap` — re-asserts the exact pinned numbers for `COLLECT_LINE_TAGS`, `EDIT_LINE_MARKERS`, and the depth-4 `if>for>while>if` fixture, all with `is_partial == false`.

    **Evidence**:
    - `cargo nextest run -p swissarmyhammer-sem -p swissarmyhammer-validators`: 633 passed, 0 failed, 0 skipped.
    - `cargo nextest run -E 'rdeps(swissarmyhammer-sem)'`: 3340 passed, 0 failed, 0 skipped (100.25s).
    - `cargo fmt --all`: clean, no changes to tracked files.
    - `cargo clippy --workspace --all-targets -- -D warnings`: clean.

    **Discovery filed separately (out of scope for this task, unrelated pre-existing code touched incidentally in probes.rs)**: ^spzf59q — `render_probe_evidence` cognitive complexity 24 (over the gate) and `run_probes`'s `probe_names: &[String]` should be `&[&str]`. Confirmed via `git diff` that neither function was touched by this task's change before filing.

    next: /review
  timestamp: 2026-08-04T08:51:12.845425+00:00
position_column: doing
position_ordinal: '8380'
title: complexity scorer's tree walk has no recursion-depth guard — pathological nesting can stack-overflow the review process
---
# Symptom

`crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs` scores a
function's cognitive complexity by recursively walking its tree-sitter parse
tree: `walk` / `walk_children` / `walk_conditional` / `walk_else` /
`walk_boolean` are ordinary Rust functions, mutually recursive, one stack frame
per AST node on the path from a function body down to its leaves.

There is no depth cap and no iterative rewrite anywhere in the file — checked
directly, no `MAX_`/depth-limit/recursion-guard of any kind exists.

# Why this matters now, not hypothetically

This probe runs on real diffs the review engine ingests, including third-party
repository content (see `mirdan/src/git_source.rs`, which parses external repo
content directly). A pathologically deep but entirely FINITE source file —
generated code, a huge chained `if`/`match`, deeply nested list/expression
literals from a code generator or a minifier — produces a parse tree deep
enough to exhaust the native call stack. That is a hard crash (stack overflow,
closer to a SIGSEGV than a graceful error), not a `not computed` result or a
caught error, and it would take down the process running the review, not just
fail one validator.

# What this is NOT

True infinite recursion is structurally impossible here: every recursive call
descends into an actual child of a tree-sitter `Node`, and parse trees are
finite and acyclic by construction, so there is no path back to an ancestor.
This is a plain stack-depth exhaustion risk on a large-but-finite input, not a
non-terminating loop.

# Current test coverage

All existing tests (`crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs`
`#[cfg(test)] mod tests`) use small hand-written fixtures. The deepest fixture
anywhere in the suite is depth 4 (`nesting_deepens_the_score_and_trips_the_depth_gate`,
`if > for > while > if`). Nothing exercises pathological or adversarial depth.

# Changes

1. Add an explicit recursion-depth cap to the walker. When a construct's
   nesting would exceed the cap, stop descending further into it: either treat
   the remainder as a bounded score contribution (documented, not silent), or
   return a `not computed` / partial result for that function the same way an
   unmapped language already does — this repo's existing convention (see
   ^xjyb2qf) is "not computed, never a silent wrong number."
2. Alternatively (and preferably, if practical), rewrite the walk as an
   explicit-stack iterative traversal so there is no cap to tune and no
   arbitrary depth after which correctness degrades. Choose whichever approach
   is simpler to get right; record which was chosen and why.
3. Whichever approach is chosen, it must never panic and never let the process
   crash on a real file — a pathologically nested file must degrade to a
   reported gap, not an unhandled abort.

# Acceptance

- A test with a source file nested far deeper than any plausible real function
  (e.g. thousands of levels) that does NOT crash the process, and returns
  either a bounded/capped score or a "not computed"/partial result — pick one
  and pin it with a test.
- A test proving the existing small fixtures (depth 1-4) are completely
  unaffected by whatever cap or rewrite is introduced — no regression on the
  already-pinned two-arm `Option` match / `tag_parser.rs` cases.
- `cargo nextest run -p swissarmyhammer-sem -p swissarmyhammer-validators`
  passes.
- `cargo fmt --all`; `cargo clippy --workspace --all-targets -- -D warnings`
  clean.

#review #bug