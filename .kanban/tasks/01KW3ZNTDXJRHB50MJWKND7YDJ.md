---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw4yx34v2czp40xnwkgzpmqm
  text: |-
    Picked up. Research complete.

    Key finding (architectural reality): there is NO prose->Tier2/3 compiler in the codebase. `compile()` (assertion.rs) only ever emits VerdictTier::Deterministic; residual criteria return CompileError::Unrecognized (no locator derivable). `ToleranceAssertion`/`JudgmentAssertion` are constructed only in tests today. `evaluate_tiered` CONSUMES pre-built tier2/3 slices; it does not compile them. So "approve() compiles tier2/3 from prose" is not implementable without building a major NLP band/tier classifier (separate large feature). Will freeze tier2/3 as empty in approve (documented) and open a follow-up for the compiler.

    Plan (the testable core of this card):
    - Golden gains `#[serde(default)] tolerance: Vec<ToleranceAssertion>` + `judgment: Vec<JudgmentAssertion>` (additive; old goldens lacking the keys deserialize to empty -> graceful degrade to Tier-1 compare).
    - New `compare_tiered(golden, received, scrubbers, embedder, judgment)` re-derives BOTH sides via `evaluate_tiered` (tier1 + golden.tolerance + golden.judgment), threading the pinned TextEmbedder + Grader panel. `compare` becomes the embedder-free Tier-1 path delegating to compare_tiered with a never-consulted embedder + empty panel (safe: empty tier2/3 never reach a resolve).
    - Drop STUB_TOLERANCE_BAND + tolerance_drift/judgment_drift stubs. Per-tier drift: Deterministic = pass-flip-or-evidence-change (unchanged); Tolerance|Judgment = pass-flip only (band/anchor logic lives in evaluate_tiered; in-band reword keeps verdict, must not read as drift).
    - TDD with stub embedder + stub/panic grader: tier2 in-band reword -> Approved; tier2 out-of-band -> Drifted; tier3 anchor-match (no grader call) -> Approved; tier3 evidence diverges + rubric-pass -> Drifted; pre-tiered golden (empty tier2/3, missing JSON keys) -> still compares.
  timestamp: 2026-06-27T16:34:16.347660+00:00
- actor: claude-code
  id: 01kw4zwk63b63cnbyz7aqevz1a
  text: |-
    Implementation landed (ledger Tier 2/3 compare wiring). Left in `doing` for review.

    Changes:
    - crates/swissarmyhammer-expect/src/ledger.rs: Golden gains `#[serde(default)] tolerance: Vec<ToleranceAssertion>` + `judgment: Vec<JudgmentAssertion>` (additive; old goldens missing the keys default empty -> graceful Tier-1 degrade). New `compare_tiered(golden, received, scrubbers, &dyn TextEmbedder, &JudgmentContext)` re-derives BOTH sides via `evaluate_tiered` over the golden's three frozen sets. `compare` is the Tier-1 path delegating to compare_tiered with a never-consulted placeholder embedder + empty panel, guarded by a `debug_assert!` that a tiered golden must use compare_tiered. Dropped STUB_TOLERANCE_BAND + tolerance_drift/judgment_drift stubs; per-tier drift now: Deterministic = pass-flip-or-evidence-change; Tolerance|Judgment = `graded_drift` (pass-flip only; band/anchor logic owned by evaluate_tiered; in-band reword keeps verdict). approve freezes tier2/3 (empty until the prose compiler lands; documented). Boxed ApprovalDecision::Write.golden to keep clippy's large_enum_variant happy after Golden grew.
    - crates/swissarmyhammer-expect/src/lib.rs: export compare_tiered.
    - crates/swissarmyhammer-tools/.../expect/mod.rs: fixture Golden literal sets the two new fields.

    TDD: added stub-embedder + stub/panic-grader tests in ledger: tier2 in-band reword -> Approved, out-of-band -> Drifted (parameterized); tier3 anchor-match -> Approved with PanicGrader (no model call); tier3 evidence-divergence + rubric-pass -> Drifted with StubGrader.calls()==1; graded_drift unit (in-band reword != drift, flip == drift); pre-tiered golden (keys stripped) deserializes + compares. RED verified: forcing graded_drift=false fails the three drift tests; reverted.

    Verification (all green):
    - cargo nextest -p expect -p tools -E 'test(ledger) or test(compare) or test(approve) or test(tier) or test(expect)': 159 passed, 0 failed.
    - cargo check --workspace: ok.
    - cargo fmt applied; cargo clippy -p expect -p tools --all-targets -D warnings: clean.

    Discovered work (follow-up created 01KW4ZD3JCRD8RR4HCG3B18DH1 / ^3b18dh1): there is NO prose->Tier2/3 compiler, so approve freezes empty tier2/3 today and production callers still use the Tier-1 `compare`. That task covers building the compiler AND re-routing ledger_state/approval_status/ledger_entry/check.rs through compare_tiered with a real embedder/grader (the `debug_assert!` guard makes the current footgun loud meanwhile). Adversarial double-check returned REVISE on exactly that future-footgun; addressed via the guard + expanded follow-up scope.
  timestamp: 2026-06-27T16:51:28.579502+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffff8680
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