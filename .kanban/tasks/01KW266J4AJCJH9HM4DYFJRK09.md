---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw3jm2c7wreb7r4f7xwpdrz2
  text: 'Picked up. Research done: assertion.rs already has CompiledAssertion::evaluate -> AssertionOutcome (Holds/Violated/Drifted/CheckpointMissing) and Locator::resolve — building evaluate.rs on top, no locator-logic duplication. types.rs has ExpectationVerdict/CriterionVerdict/Evidence/Reliability. observe.rs has received_path + EXPECT_DIR; will add golden_path alongside (refactor safe-join). Plan: pure evaluate(obs,&[CompiledAssertion]) maps each outcome to CriterionVerdict (drift = score None + loud reason, distinct from value-fail score 0.0); evaluate_spec(spec,obs) compiles fresh (stopgap until golden freezes assertions in ^4v1sdvy) then evaluates. Ops load received/golden file + spec criteria, call evaluate_spec, return ExpectationVerdict JSON; handle missing file gracefully.'
  timestamp: 2026-06-27T03:40:23.303231+00:00
- actor: claude-code
  id: 01kw3k91gaebf9apa7b6rbe542
  text: |-
    Done, left in `doing` for review. Implemented:
    - crates/swissarmyhammer-expect/src/evaluate.rs (NEW): pure evaluate(obs,&[CompiledAssertion]) + evaluate_assertion + evaluate_spec(spec,obs). Replays via CompiledAssertion::evaluate (no locator-logic dup). Drift = score None + STRUCTURAL_DRIFT_REASON prefix; value-fail = score Some(0.0). 13 unit tests incl. pass/fail/drift/missing-checkpoint/exit/regex/purity.
    - assertion.rs: impl Display for BoundValue (evidence snippets).
    - observe.rs: added golden_path, refactored received_path to share expect_artifact_path safe-join (+2 tests).
    - tools/expect/mod.rs: wired observation/observations/golden/goldens evaluate -> shared evaluate_op (EvaluateSource Received/Golden); load received/golden file, run evaluate_spec, return ExpectationVerdict JSON; missing file -> graceful "missing" status. Reads .expect/goldens/<path>.golden.json (golden store lands in ^4v1sdvy). Converted those 4 op structs to manual impls w/ shared SCOPE_PARAMS (was OBSERVE_PARAMS). +3 op tests.

    Verification (all green): cargo nextest -p swissarmyhammer-expect -p swissarmyhammer-tools -E 'test(evaluate) or test(expect)' = 67 passed; cargo check --workspace OK; cargo fmt applied; clippy -p swissarmyhammer-expect -p swissarmyhammer-tools -D warnings clean. Verified drift test has teeth via mutation (score None->Some(0.0) -> drift test fails). double-check agent: PASS (two non-blocking cosmetic notes: all-skipped-criteria verdict satisfies() trivially true = defensible Tier-1 "no objection"; fail reason for invariants names the left locator — data correct).
  timestamp: 2026-06-27T03:51:50.538721+00:00
depends_on:
- 01KW265D8SHMBFYBCZ5QEMBVQ0
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffffff280
project: expect
title: evaluate (pure) Tier 1 + observation/golden evaluate ops
---
## What
Implement the pure `evaluate` function for Tier 1 (deterministic) and expose it as the `observation evaluate` AND `golden evaluate` ops. Per `ideas/expect.md` §"The Verdict Ladder" and the `evaluate` signature.

- New `crates/swissarmyhammer-expect/src/evaluate.rs`:
  - `evaluate(obs: &Observation, criteria: &[CompiledAssertion]) -> ExpectationVerdict` — PURE, touches no system, re-runnable for free. Replays frozen/compiled assertions (no recompile during evaluate).
  - Tier 1 resolution: exact / regex / numeric / exit-code / file-state via the locator + op; produces a `CriterionVerdict { tier: Deterministic, pass, evidence, reason }`. A locator that no longer binds ⇒ a structural-drift verdict (distinct from a plain fail), surfaced loudly.
  - The verdict is structured (never a bare boolean) so it can drive the next agent edit: include the evidence slice and a reason per criterion.
  - "Pure means no SUT, not uniformly deterministic" — Tier 1 here is fully deterministic with no model.
- Wire ops in `tools/expect/mod.rs` (replace stubs):
  - `observation evaluate <scope>` / `observations evaluate` — load the stored received Observation + its compiled assertions, run `evaluate`, return the `ExpectationVerdict` as JSON. Re-judge a stored observation without re-running.
  - `golden evaluate <scope>` / `goldens evaluate` — re-grade the APPROVED golden observation against the current/edited criteria (the design flow "edited a criterion — re-grade without re-running"). Same pure `evaluate`, source = golden observation; no SUT driven.

## Acceptance Criteria
- [ ] `evaluate` over a fixture Observation + Tier-1 assertions returns a per-criterion verdict with evidence and reason; passes/fails correctly.
- [ ] A locator that fails to bind yields a structural-drift verdict, not a silent fail.
- [ ] `evaluate` invokes no process and no model (assert via a no-side-effect fixture).
- [ ] `observation evaluate <scope>` re-judges the stored received observation without re-running.
- [ ] `golden evaluate <scope>` re-grades the approved golden observation against current criteria without re-running the system (returns the verdict for the baseline under edited criteria).

## Tests
- [ ] `crates/swissarmyhammer-expect/src/evaluate.rs` unit tests: pass case, fail case, structural-drift (locator stopped binding), exit-code and regex ops.
- [ ] Tools op tests: `observation evaluate` returns the expected verdict JSON from a stored received file; `golden evaluate` re-grades a stored golden against an edited criterion set.
- [ ] `cargo nextest run -p swissarmyhammer-expect evaluate` passes.

## Workflow
- Use `/tdd`.