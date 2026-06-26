---
assignees:
- claude-code
depends_on:
- 01KW267C7WHM0ETTDY64V1SDVY
- 01KW267YT5YS3CXDCWE1YG9JR0
position_column: todo
position_ordinal: b780
project: expect
title: Drift detection + ledger states + expectations list
---
## What
Compute per-expectation ledger state from the compare, queue unapproved drift, and surface it via `expectations list`. Per `ideas/expect.md` §"The Drift Ledger" and the runtime ledger enum.

- In `crates/swissarmyhammer-expect/src/ledger.rs`:
  - `ledger_state(expectation, golden?, received?) -> LedgerState`:
    - `new` — no golden yet.
    - `approved` — `evaluate(received)` matches `evaluate(golden)` within tolerance.
    - `drifted` — verdict changed, awaiting human approval (surface old-vs-new).
    - `stale` — the `*.expect.md` was edited since its golden was approved (detect via a content/criteria hash stored in the golden).
  - Build the unapproved-drift queue; `expectations list` surfaces pending old-vs-new diffs FIRST, then the rest, each annotated with its ledger state.
- Wire `expectations list` op in `tools/expect/mod.rs` to return specs + ledger state (new/approved/drifted/stale).

## Acceptance Criteria
- [ ] No golden ⇒ `new`; matching verdicts ⇒ `approved`; changed verdict ⇒ `drifted`; spec edited after approval ⇒ `stale`.
- [ ] `expectations list` returns every spec with its ledger state, drifted ones listed first with old-vs-new evidence.
- [ ] Drift is computed as `evaluate(received)` vs `evaluate(golden)` (re-derived both sides), not a stored verdict.

## Tests
- [ ] `crates/swissarmyhammer-expect/src/ledger.rs` tests for each of the four states incl. the stale-after-edit hash check.
- [ ] Tools op test: `expectations list` JSON includes ledger state and orders drifted-first.
- [ ] `cargo nextest run -p swissarmyhammer-expect drift` passes.

## Workflow
- Use `/tdd`.