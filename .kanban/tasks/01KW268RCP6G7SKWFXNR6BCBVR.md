---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw3xzk2j35rxq41bt7cq651v
  text: |-
    Implemented drift detection + ledger states + expectations list.

    Engine (crates/swissarmyhammer-expect/src/ledger.rs):
    - Added `spec_hash(&Expectation) -> String`: sha256 of the criteria texts, length-prefixed for domain separation, `sha256:` prefixed. Checkbox state excluded (review bookkeeping, not grading content). sha2 added as a workspace dep.
    - Added `spec_hash: String` field to `Golden` (additive, public); set in `approve()` via `spec_hash(spec)`.
    - `ledger_state(spec, golden?, received?, scrubbers) -> LedgerState`: precedence New (no golden) > Stale (golden.spec_hash != spec_hash(spec)) > {Drifted|Approved via compare()} ; no received + unedited spec => Approved. Stale outranks drift.
    - `LedgerEntry { path, state, comparison }` + `ledger_entry(...)` (attaches old-vs-new compare() only when Drifted) + `ledger_queue(entries)` (stable sort, drift rank 0 leads).
    - Drift re-derived both sides via existing compare(), never a stored verdict.

    Tool (crates/swissarmyhammer-tools/src/mcp/tools/expect/mod.rs):
    - Replaced the `list expectations` stub with `expectations_list`: loads golden+received per spec, builds ledger_entry, orders drifted-first via ledger_queue. Drifted rows carry the comparison; others omit it.

    Tests (TDD, RED watched via missing-symbol compile failure then GREEN): four ledger states + stale-outranks-drift + spec_hash stability/edit/boundary + ledger_entry evidence-only-when-drifted + ledger_queue ordering; tool test expectations_list_returns_ledger_state_and_orders_drifted_first.

    Verification: cargo nextest -p expect -p tools -E 'test(drift) or test(ledger) or test(expect)' => 126 passed; cargo check --workspace ok; cargo fmt applied; clippy -D warnings clean.
  timestamp: 2026-06-27T06:58:55.186508+00:00
- actor: claude-code
  id: 01kw3ybxm5pya130s843h8smht
  text: |-
    Adversarial double-check verdict: REVISE, one Medium finding (fixed).

    Finding 1 (fixed): `ExpectationsList` was built via the `#[operation]` macro (fieldless struct => empty parameters()), so `list expectations` honored scope/tag in the handler but never advertised them in the CLI/MCP schema — unlike sibling `ObservationsList`/`GoldensList` which use manual Operation impls returning SCOPE_PARAMS. Converted `ExpectationsList` to a manual `impl Operation` returning `SCOPE_PARAMS`. Added two regression guards: `expectations_list_declares_the_scope_inputs` (asserts the op advertises scope+tag) and a scoped-dispatch assertion in the main op test (scope "drifted" => count 1).

    Finding 2 (acknowledged, by design): spec_hash keys only on `## Then` criteria text (matches the task's "criteria hash" wording and is documented). Editing non-criteria content surfaces via re-run drift, not stale.

    Re-verified: 127 targeted tests pass; clippy -p expect -p tools --all-targets -D warnings clean; cargo fmt applied; cargo check --workspace ok. Task left in `doing` for /review.
  timestamp: 2026-06-27T07:05:39.205854+00:00
depends_on:
- 01KW267C7WHM0ETTDY64V1SDVY
- 01KW267YT5YS3CXDCWE1YG9JR0
position_column: doing
position_ordinal: '8280'
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