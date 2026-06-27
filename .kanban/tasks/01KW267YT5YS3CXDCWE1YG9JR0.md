---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw3sj24wbc3mzk8jk7wp536q
  text: 'Picked up. Dependencies (ledger store+scrubbers+compare, assertion compiler) are landed and merged. Plan: add to ledger.rs a pure approval API — approve() (scrub received, freeze compiled assertions reusing compile()''s self-verification as the hallucination guard, with hand-edit preservation keyed on criterion prose), approval_diff() (binding view: value ← locator), approval_status() + ApproveMode::selects() (missing/changed/all selection), and decide_approval(spec, golden, received, mode, grading, ci, scrubbers) -> ApprovalDecision {Write|Skipped|RefusedInCi}. CI is injected as a bool into the core (no ambient env). Wire approve observation/observations ops in tools/expect/mod.rs: --missing/--changed/--all bool flags (none = preview requiring confirmation), CI read from env CI=true at the op edge, two-pass atomic write (build all decisions; any RefusedInCi or compile hallucination fails the whole pass, writing nothing). TDD.'
  timestamp: 2026-06-27T05:41:37.564308+00:00
- actor: claude-code
  id: 01kw3tege5vk70hh4r2zgr16f5
  text: |-
    Implemented and green; left in doing for review.

    Files changed:
    - crates/swissarmyhammer-expect/src/ledger.rs — approval API: approve() (scrubs received obs, freezes compiled assertions against the SCRUBBED observation, reuses compile()'s self-verification as the hallucination guard via freeze_assertions; preserves a reviewer hand-edited locator keyed on criterion prose, recompiles when prose changes), approval_diff()+ApprovalBinding (binding view, render "40 ← $.total", hand_edited flag = frozen != fresh compile), approval_status(), ApproveMode+selects() (missing/changed/all data table), ApprovalStatus, decide_approval()->ApprovalDecision{Write|Skipped|RefusedInCi}, ApproveError. +14 unit tests.
    - crates/swissarmyhammer-expect/src/lib.rs — re-exports.
    - crates/swissarmyhammer-tools/src/mcp/tools/expect/mod.rs — wired approve observation/observations ops; APPROVE_PARAMS (scope/tag + --missing/--changed/--all bool flags built from SCOPE_PARAMS); ci_enabled() reads env CI=true at the op edge (consts CI_ENV_KEY/CI_ENABLED_VALUE); two-pass approve_write (all decisions built first; any RefusedInCi/compile error fails the pass before any write); approve_preview (no flag = requires_confirmation, writes nothing). +8 op tests.

    CI-refusal + strict-first-run: the CI flag is INJECTED as a bool into the pure decide_approval (no ambient env in core). A selected spec under ci=true returns RefusedInCi -> op returns a hard invalid_params error and writes nothing; a `new` expectation under ci is likewise RefusedInCi (verified no golden lands on disk) — a green baseline is never minted in CI, only locally by observe+approve. Op write-path tests inject ci=false directly so the ambient CI env on a CI runner cannot make them flaky; one #[serial(env)] test covers ci_enabled()'s env read.

    Hallucination guard: a criterion whose locator can't bind/pass its source observation (e.g. literal "$40" vs received total=50) is rejected at approve (ApproveError::Compile) — so a literal-drift that violates its own criterion is unapprovable; a legitimately approvable drift is an invariant whose incidental value moved but relationship holds.

    Adversarial double-check: PASS on all four acceptance criteria + conventions; one low docstring-atomicity nit (fixed — scoped the "writes nothing" claim to CI/compile refusals vs raw IO faults); one informational Tier-2/3-diff note left as a follow-up (out of scope, compiler is Tier-1-only).

    Verification (all green): cargo nextest -p swissarmyhammer-expect -p swissarmyhammer-tools -E 'test(approve) or test(ledger) or test(expect)' = 102 passed; cargo check --workspace ok; cargo fmt applied; cargo clippy -p swissarmyhammer-expect -p swissarmyhammer-tools -- -D warnings clean.
  timestamp: 2026-06-27T05:57:09.701207+00:00
depends_on:
- 01KW267C7WHM0ETTDY64V1SDVY
- 01KW265D8SHMBFYBCZ5QEMBVQ0
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffffff580
project: expect
title: 'observation approve: freeze assertions, diff, human gate, strict first-run'
---
## What
Promote a received observation to golden, freezing the compiled assertions, behind a human gate over a reviewable diff. Per `ideas/expect.md` §"The Drift Ledger" (`approve`) and §"Compilation freezes into the golden".

- In `crates/swissarmyhammer-expect/src/ledger.rs` (+ tools op):
  - `approve(scope, mode)` promotes the last `received` observation to `.expect/goldens/<path>.golden.json`, scrubbed, with the compiled assertion set FROZEN alongside it (compilation happens at approve, bound against the approved observation; self-verifying per the compiler task).
  - The approve **diff** shows the binding ("$40 ← `$.total`"), not just the value, so a mis-compiled locator is caught at review. Allow a reviewer hand-edit of a locator; a hand-edit is bound to the criterion prose (changing the prose discards+recompiles+re-reviews).
  - Granular modes mirroring `--update-snapshots`: `--missing` (brand-new only), `--changed` (only drifted), `--all` (bulk).
  - **Strict first-run**: a `new` expectation (no golden) cannot pass in CI — you can never mint a green baseline in CI; the golden is created locally by observe+approve and committed.
  - **`CI=true` never auto-approves**: an unapproved drift is always a hard failure; approve never silently writes in CI.
- Wire `observation approve` / `observations approve` ops in `tools/expect/mod.rs` with the mode flags.

## Acceptance Criteria
- [ ] `observation approve <scope>` writes the scrubbed golden + frozen assertions; the diff output shows criterion→binding, not just values.
- [ ] `--missing`/`--changed`/`--all` select the right subset; default requires explicit confirmation.
- [ ] With `CI=true`, approve refuses to write (hard failure on unapproved drift), and a `new` expectation fails rather than auto-baselining.
- [ ] A frozen assertion that fails to bind/pass against the observation it was compiled from is rejected (no hallucinated locator reaches the golden).

## Tests
- [ ] `crates/swissarmyhammer-expect/src/ledger.rs` tests: approve writes golden+frozen assertions; `--missing`/`--changed` selection; `CI=true` refuses; new-in-CI fails.
- [ ] Test the diff renders the binding.
- [ ] `cargo nextest run -p swissarmyhammer-expect approve` passes.

## Workflow
- Use `/tdd`.