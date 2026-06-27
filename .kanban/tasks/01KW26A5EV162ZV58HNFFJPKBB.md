---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw469075zdz7bq0a39b6xcj7
  text: |-
    Picked up. Research done. Design:
    - observe.rs: extract observe_round (arrange+act+observe against a provisioned sut); add observe_repeated(spec, adapter, config) -> Vec<Observation> honoring isolation (Shared=provision once + re-arrange Given per run; Fresh=full lifecycle per run) and a resolved_repeat() that defaults to 1 run for mechanically-driven surfaces and >=2 when an agent drives (via resolves_mechanically), clamped to the pass^k floor. Each run re-arranges Given.
    - evaluate.rs: add evaluate_repeated(spec, &[Observation]) -> ExpectationVerdict composing the multi-run Reliability (required from policy, runs = per-run overall pass; representative criteria = last run).
    - types.rs: Reliability::passed() count for the per-run spread message.
    - check.rs: observe seam changes to FnMut -> Result<Vec<Observation>>; uses evaluate_repeated; golden-compares the last observation; Failed message shows spread.
    - tools mod.rs: observe_for_check returns observe_repeated(...), writes last received.
    Escalation/Tier-3: check only runs Tier-1 evaluate_spec today, so escalation queue is not in this path; deferring (task says "if relevant").
  timestamp: 2026-06-27T09:23:52.165545+00:00
- actor: claude-code
  id: 01kw46xtsw6sf3m7k2w5qbxw14
  text: |-
    Implemented pass^k reliability + repeat runs + isolation:fresh. Files changed:
    - crates/swissarmyhammer-expect/src/observe.rs: extracted observe_round; added observe_repeated() -> Vec<Observation>, resolved_repeat(), drives_mechanically(), observe_shared/observe_fresh. Shared isolation provisions once + re-arranges Given per run; Fresh provisions per run. resolved_repeat: mechanical default 1, agent default >=2, clamped to the pass^k floor.
    - crates/swissarmyhammer-expect/src/evaluate.rs: evaluate_repeated(spec, &[Observation]) composing multi-run Reliability.
    - crates/swissarmyhammer-expect/src/types.rs: Reliability::passed() spread count.
    - crates/swissarmyhammer-expect/src/check.rs: observe seam now FnMut -> Result<Vec<Observation>>; uses evaluate_repeated; golden-compares last run; Failed message appends "(P/N runs passed)" spread.
    - crates/swissarmyhammer-expect/src/lib.rs: re-exports.
    - crates/swissarmyhammer-tools/src/mcp/tools/expect/mod.rs: observe_for_check returns observe_repeated(...), writes last as received.

    Escalation/Tier-3: check runs only Tier-1 evaluate_spec today, so the escalation queue is not in this path — deferred (task says "if relevant").

    Verification (all green): cargo nextest -p swissarmyhammer-expect -E 'test(reliability) or test(observe) or test(check) or test(passk)' = 41 passed; full -p swissarmyhammer-expect = 210 passed; -p swissarmyhammer-tools -E 'test(expect)' = 70 passed; cargo clippy -p swissarmyhammer-expect -- -D warnings clean; cargo check --workspace OK; cargo fmt applied. double-check agent verdict: PASS.

    Left in doing for /review.
  timestamp: 2026-06-27T09:35:14.748783+00:00
depends_on:
- 01KW263S53NJ1YWNHGPYTTWEEC
- 01KW2694VSQ17BJ1H8QWB0X0C1
position_column: doing
position_ordinal: '8280'
project: expect
title: pass^k reliability + repeat runs + isolation fresh
---
## What
Make flakiness a first-class, declared property: run an expectation k times and require all k to pass, with each run re-arranging its `Given`. Per `ideas/expect.md` §"Reliability and Non-Determinism" and §"Provisioning and Isolation".

- In `crates/swissarmyhammer-expect/src/observe.rs` / `check.rs`:
  - Honor `reliability: pass^k` (and `repeat`): run `observe` k times; the expectation passes iff all k runs pass. Report the per-run spread (a 2-of-3 flake is visible, not hidden behind an average) in `Reliability`.
  - **Default repeat ≥2 only when an agent drives** (the runtime fallback). Mechanically-driven surfaces (cli/http/file/db/a11y browser/gui) are deterministic ⇒ run once by default.
  - **Re-arranged Given per run**: each repeated `observe` must re-establish `Given` state (shared SUT across a check) or the repeats aren't independent and pass^k is theater. Enforce/encourage via the arrange step.
  - **`isolation: fresh`**: provision a dedicated SUT instance for an expectation that needs a pristine system, instead of the shared per-check instance.
- Escalation-queue surfacing of low-confidence criteria threads through here (from the Tier 3 task).

## Acceptance Criteria
- [ ] `reliability: pass^3` runs observe 3× and fails if any run fails; the report shows the per-run spread.
- [ ] A deterministic cli spec defaults to a single run; an agent-driven spec defaults to ≥2.
- [ ] Each repeated run re-arranges `Given` (assert the arrange step runs per observe).
- [ ] `isolation: fresh` provisions a dedicated instance (assert distinct provision from the shared one).

## Tests
- [ ] Tests: pass^3 all-pass⇒pass; one-fail⇒fail with spread; deterministic⇒1 run; agent-driven⇒≥2; fresh isolation provisions separately (stub adapter counting provisions).
- [ ] `cargo nextest run -p swissarmyhammer-expect reliability` passes.

## Workflow
- Use `/tdd`.