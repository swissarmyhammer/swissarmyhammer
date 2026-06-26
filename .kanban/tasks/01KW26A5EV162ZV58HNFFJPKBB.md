---
assignees:
- claude-code
depends_on:
- 01KW263S53NJ1YWNHGPYTTWEEC
- 01KW2694VSQ17BJ1H8QWB0X0C1
position_column: todo
position_ordinal: ba80
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