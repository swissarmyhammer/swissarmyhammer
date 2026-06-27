---
assignees:
- claude-code
position_column: todo
position_ordinal: c780
project: expect
title: Compile residual criteria into frozen Tier 2/3 assertions at approve
---
## What
Build the prose -> Tier 2/3 compile path so `approve()` actually freezes `ToleranceAssertion`/`JudgmentAssertion` onto the `Golden`, instead of leaving the new `Golden.tolerance`/`Golden.judgment` sets empty AND re-route the production ledger consumers through `compare_tiered` so a tiered golden is graded with the pinned embedder + grader.

## Why (discovered during 01KW3ZNTDXJRHB50MJWKND7YDJ)
The ledger compare is now fully tiered: `compare_tiered()` re-derives both sides via `evaluate_tiered` with the pinned embedder + grader, and the Golden carries `tolerance`/`judgment` (additive, serde default). BUT there is no compiler that turns a residual `## Then` criterion into a Tier 2/3 assertion:
- `assertion::compile()` only ever emits `VerdictTier::Deterministic`; a residual returns `CompileError::Unrecognized` (no locator derivable).
- `evaluate_tiered` CONSUMES pre-built tier2/3 slices; it does not compile them.
- So `approve()` currently freezes `tolerance: vec![]`, `judgment: vec![]` (documented as awaiting this task). Until then, real goldens never exercise tier2/3 in compare; only directly-constructed test goldens do.

Second gap (raised by adversarial review): every PRODUCTION ledger entry point — `ledger_state`, `approval_status`, `ledger_entry` (ledger.rs) and `check.rs` — calls the Tier-1-only `compare()`, which injects a never-consulted placeholder embedder + empty grader panel. Harmless while tier2/3 are always empty; but once this task populates them, those callers would SILENTLY mis-grade tier2/3 (empty-vector cosine = 0 fails both sides, so a real divergence reads as no drift). A `debug_assert!` guard was added to `compare()` in 01KW3ZNTDXJRHB50MJWKND7YDJ to make that footgun loud — this task must close it.

## Acceptance
- A residual criterion (one that fails Tier-1 compile) is classified into Tier 2 (tolerance band: numeric / semantic / near-string + anchor + locator) or Tier 3 (judgment: locator + anchor evidence + sim_threshold + rubric), bound against the approved observation.
- `approve()` freezes all three tiers via a single compile-bundle path shared with any `evaluate_tiered` caller.
- `ledger_state` / `approval_status` / `ledger_entry` / `check.rs` thread a real pinned `TextEmbedder` + `Grader` panel and route through `compare_tiered` (not `compare`), so the `debug_assert!` guard in `compare()` never trips in production.
- `compare_tiered` then drives real (non-empty) tier2/3 sets end-to-end from approve -> compare.
- Tests: approve a spec with a tolerance criterion and a judgment criterion; assert the frozen sets are populated and that compare detects drift on a real (approve-produced) golden with stub embedder/grader.