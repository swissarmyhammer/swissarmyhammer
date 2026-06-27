---
assignees:
- claude-code
position_column: todo
position_ordinal: c580
project: expect
title: Wire real Tier 2/3 ledger compare (drop tolerance/judgment stubs)
---
## What
Replace the `tolerance_drift`/`judgment_drift` **stubs** in `crates/swissarmyhammer-expect/src/ledger.rs` with the real Tier 2/3 drift semantics now that the Tier 2 (`ToleranceAssertion`/`ToleranceBand`) and Tier 3 (`grader::JudgmentAssertion` + `Grader`) evaluation primitives exist.

Deferred from 01KW2694VSQ17BJ1H8QWB0X0C1 (Tier 3) because it was explicitly "if straightforward" and is not: it requires architectural changes, not a local edit.

## Why it is not local
- `compare()` re-derives verdicts via `evaluate(obs, golden.assertions)`, and `Golden.assertions` is `Vec<CompiledAssertion>` (Tier 1 only). The golden does not yet store `ToleranceAssertion`/`JudgmentAssertion`, so Tier 2/3 verdicts never appear in the compare today.
- A real `tolerance_drift` needs the golden's score + band (drift if the score leaves the band); the band is not carried on `CriterionVerdict`.
- A real `judgment_drift` needs embedding similarity between approved (golden) and received evidence vs the pinned threshold — i.e. a `TextEmbedder` threaded through `compare`/`compare_criterion`/`ledger_state`/`ledger_entry`/`ledger_queue`/`approval_*`.

## Acceptance
- Golden stores Tier 2/3 assertions; `compare` re-derives via `evaluate_tiered` with the pinned embedder (and grader where applicable).
- `tolerance_drift`: drift iff the re-derived score leaves the golden's band (no more `STUB_TOLERANCE_BAND` EPSILON).
- `judgment_drift`: drift iff received-vs-approved evidence similarity falls below the pinned `GradingPins.similarity_threshold` (no more exact-evidence-equality stub).
- Tests with stub embedder/grader for both drift rules.