---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw54nz8f7gkt03z4gmheq4mf
  text: |-
    Picked up. Research complete across assertion.rs/evaluate.rs/grader.rs/ledger.rs/check.rs/config.rs + tool layer expect/mod.rs.

    Plan:
    - assertion.rs: expose resolve_checkpoint + word_present as pub(crate); add locate_text_evidence(text,state)->Option<(Locator,BoundValue)> (key-token-matched string scalar, else sole string scalar in checkpoint JSON).
    - evaluate.rs: add compile_tiered(criterion,obs,threshold)->Result<CompiledTier,CompileError> + CompiledTier enum {Deterministic|Tolerance|Judgment}. Tier1 first (compile); on Unrecognized, locate textual evidence; subjective cue (JUDGMENT_CUES) -> Tier3 Judgment (rubric=prose, anchor=evidence, sim_threshold), else Tier2 Tolerance(Semantic{threshold}). Self-verify the locator re-binds to the frozen anchor.
    - ledger.rs: approve() freezes all 3 tiers via freeze_tiers (shares compile_tiered). Add GradingSeam{embedder,judgment} bundle. Reroute ledger_state/approval_status/ledger_entry/decide_approval to take seam + call compare_tiered. Drop the debug_assert in compare (keep compare as the Tier-1-only convenience; production now routes tiered goldens through compare_tiered).
    - check.rs: CheckOptions gains seam; check_one routes through compare_tiered.
    - lib.rs: export compile_tiered, CompiledTier, GradingSeam.
    - tool layer: construct a placeholder seam (PlaceholderEmbedder warns on use; empty grader panel) at approve_op/expectations_list/check_op. Real pinned embedder/grader wiring into the tool layer remains a noted follow-up (no model/GPU per task scope).
    - TDD with stub embedder/grader at engine level.
  timestamp: 2026-06-27T18:15:14.447076+00:00
- actor: claude-code
  id: 01kw566nm9djppdgw24ycjhnfc
  text: |-
    Implementation landed; left in `doing` for review.

    What changed (residual -> Tier2/Tier3 compile + freeze + re-route):
    - assertion.rs: exposed `resolve_checkpoint`/`word_present` as pub(crate); added `locate_text_evidence(text,state)` (key-token-matched string scalar, else the sole string scalar in the checkpoint JSON) — the shared evidence locator for both residual tiers.
    - evaluate.rs: added `compile_tiered(criterion,obs,threshold) -> Result<CompiledTier,CompileError>` + `CompiledTier{Deterministic|Tolerance|Judgment}`. Tier 1 (`compile`) wins; on `Unrecognized`, `compile_residual` locates textual evidence, self-verifies the locator re-binds to the frozen anchor (binding-only, like Tier 1), then routes by `is_subjective` (JUDGMENT_CUES): subjective -> Tier 3 judgment (rubric = prose, anchor = evidence, sim_threshold = effective cutoff); else -> Tier 2 tolerance (Semantic band). No-evidence residual stays `Unrecognized` for the doctor gate.
    - ledger.rs: `approve`/`freeze_tiers` now freeze all three tiers via `compile_tiered` (threshold = per-spec override else grading.similarity_threshold). Added `GradingSeam{embedder,judgment}`. Re-routed `ledger_state`/`approval_status`/`ledger_entry`/`decide_approval` to take the seam and call `compare_tiered`. Dropped the `debug_assert!` in `compare` (kept as the documented Tier-1-only convenience; no production caller feeds it a tiered golden).
    - check.rs: `CheckOptions` gained `seam`; `check_one` routes ledger_state + drift comparison through `compare_tiered`.
    - lib.rs: exported `compile_tiered`, `CompiledTier`, `GradingSeam`.
    - tools/expect/mod.rs: threaded a placeholder seam (`PlaceholderEmbedder` -> empty vec + tracing::warn; empty grader panel) through approve_op/expectations_list/check_op. Real pinned embedder/grader tool-layer wiring deferred to follow-up ^hatnq5h (01KW55PG8RNYCQB7V75HATNQ5H) per task scope ("do NOT block on standing up a real model; inject the seam and note what remains").

    TDD: new engine tests are RED-verified then GREEN: compile_tiered Tier1/Tier2/Tier3 classification + self-verify + no-evidence-Unrecognized (evaluate.rs); approve freezes tier2+tier3 with self-verify, and compare_tiered detects Tier2 AND Tier3 drift end-to-end on an approve-PRODUCED golden with stub embedder+stub grader (ledger.rs); pre-tiered goldens still load (existing test).

    Verification (all green):
    - cargo nextest -p expect -p tools -E 'test(approve) or test(compile) or test(tier) or test(ledger) or test(expect)': 176 passed, 0 failed.
    - cargo check --workspace: ok.
    - cargo fmt applied; cargo clippy -p expect -p tools --all-targets -D warnings: clean (added a justified `#[allow(clippy::too_many_arguments)]` on decide_approval — 8 distinct injected policy inputs).

    Adversarial double-check: REVISE with two ADVISORY findings.
    1) Tool layer uses a placeholder seam, not real pinned embedder/grader (AC3 "real ... in production"). JUSTIFIED/DEFERRED: explicitly sanctioned by the task scope; engine path is fully real + tested; follow-up ^hatnq5h created and strengthened to flag the silent-false-negative risk (a tiered golden currently reads Approved in the tool layer until real wiring lands).
    2) Perception verbs in JUDGMENT_CUES mis-route literal-text criteria to Tier 3. ADDRESSED: removed ambiguous `reads`/`looks`/`sounds` from JUDGMENT_CUES so they default to the cheaper Tier 2 semantic band (kept clearly-subjective `feels`/`seems`); documented the rationale. (Did not add NearString-band residual selection — Semantic is a faithful default; the ToleranceBand enum already supports all bands per AC1.)
  timestamp: 2026-06-27T18:41:50.217566+00:00
position_column: doing
position_ordinal: '8280'
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