---
assignees:
- claude-code
depends_on:
- 01KW267C7WHM0ETTDY64V1SDVY
- 01KW265D8SHMBFYBCZ5QEMBVQ0
position_column: todo
position_ordinal: b580
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