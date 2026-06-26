---
assignees:
- claude-code
depends_on:
- 01KW265D8SHMBFYBCZ5QEMBVQ0
position_column: todo
position_ordinal: b280
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