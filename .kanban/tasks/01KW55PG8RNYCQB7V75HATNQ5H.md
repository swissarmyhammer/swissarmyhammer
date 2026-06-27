---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw57saxyhwsapjs2aafey53g
  text: |-
    Picked up. Research done across review_op.rs (embedder factory: async default_embedder_factory + shared OnceCell cache, model_embedding::TextEmbedder is ASYNC embed_text), expect_op.rs (AgentFactory/run_*_request ACP seam), grader.rs/evaluate.rs (swissarmyhammer_expect::TextEmbedder is SYNC embed()), ledger.rs (GradingSeam, compare_tiered, graded_drift), check.rs.

    Key finding: for the ledger's binary drift (graded_drift = golden.pass != received.pass), the Tier-3 judgment pass is decided ENTIRELY by the embedder's anchor-similarity gate — the model panel only refines the reason/confidence, never the pass/fail. So the EMBEDDER is the load-bearing fix for faithful tolerance+judgment drift; the grader panel is secondary.

    Impedance mismatch: the 3 tool ops (approve_op/expectations_list/check_op) are SYNC; the platform embedder is async-load+async-embed; the grader needs a [model]-config grading model distinct from the driver, which would need new cross-crate server factory wiring.

    Plan: (1) Implement the INTERIM LOUD-AND-SAFE GUARD first — tiered golden (non-empty tolerance/judgment) + only placeholder seam ⇒ non-approved "uncheckable" status at all three call sites; drop the warn. Non-model test runs unconditionally. This fully closes the silent-false-negative footgun. (2) Layer the real pinned embedder adapter (model_embedding::TextEmbedder → expect::TextEmbedder, bridged sync via Handle::block_on under spawn_blocking, mirroring review's default_embedder_factory + shared cache) + grader-panel resolution from [model] config (driver excluded) when it can be wired green; GPU-gated end-to-end test. Verifying green at each step.
  timestamp: 2026-06-27T19:09:30.430374+00:00
- actor: claude-code
  id: 01kw59nzqq7kamcpf1617gdecn
  text: |-
    Implementation landed (all green). Files changed:
    - crates/swissarmyhammer-tools/src/mcp/tools/expect/grading.rs (NEW): `ExpectEmbedder` adapts the async platform `model_embedding::TextEmbedder` (the SAME model review loads via review_op::default_embedder_factory) to the engine's SYNC `swissarmyhammer_expect::TextEmbedder`, driving `embed_text` via `Handle::block_on` from inside `spawn_blocking`. Carries a `failed: Cell<bool>` so a per-call embed failure is caught (take_failed) and escalated rather than silently graded on an empty vector.
    - crates/swissarmyhammer-tools/src/mcp/tools/expect/mod.rs: ExpectTool gained `embedder_factory: Option<review_op::EmbedderFactory>` + `with_embedder_factory` + async `resolve_embedder`. `register_expect_tools` wires `review_op::default_embedder_factory()` (production now grades tiered goldens with the real pinned embedder). The three ops (expectations_list/approve_op[preview+write]/check_op) became async and run grading in spawn_blocking with the real adapter when loaded; INTERIM GUARD (`golden_requires_grading` + `UNCHECKABLE_STATUS`) escalates a tiered golden to a NON-approved `uncheckable` row when no real embedder is available OR an embed failed mid-grade. Dropped the placeholder warn.

    How it mirrors review: reuses review_op's EmbedderFactory + default_embedder_factory (shared OnceCell cache, same platform model) verbatim; the embedder seam is injected exactly like review's. Where review's engine awaits the embedder, expect bridges sync via block_on (its grading trait is sync).

    Faithful-drift insight: the Tier-3 judgment `pass` is decided entirely by the embedder's anchor-similarity gate (the model panel only refines the reason). So the real embedder makes BOTH tolerance and judgment drift faithful at the ledger level with an empty live panel (diverged => escalation => non-pass => drift).

    Tests: 4 non-model guard tests + 1 mock-embedder regression test (failed embed => uncheckable, not approved) run unconditionally; 1 GPU-gated end-to-end test (real embedder grades a diverged tiered golden => `drifted`) — it actually RAN on this Metal box (~13s). Verification: nextest selection = 214 passed; clippy -D warnings clean; cargo check --workspace clean; cargo fmt applied.

    REMAINING model-gated slice (design-decision blocker, reported): the LIVE Tier-3 grader PANEL consulting the [model] default+panel grading models via a model call distinct from the driver. That needs a cross-crate grader factory (model-call backend, like swissarmyhammer-agent's review_agent_factory) + server wiring (set_expect_factories) — a design decision I did not guess. Faithful DRIFT detection is already achieved by the real embedder + escalation; the panel would only enrich the judgment REASON (drift vs fail vs panel-disagreement). The expect tool's create-agent factory is also still unwired in production (pre-existing, separate gap).

    Adversarial double-check found and I fixed a real bug: once the real embedder loads, a per-call embed failure degraded to an empty vector that scored cosine 0 on BOTH sides => silent Approved in the model-loaded-but-call-fails window. Now caught via the failed flag (covered by the new mock regression test).
  timestamp: 2026-06-27T19:42:37.815287+00:00
- actor: claude-code
  id: 01kw59xrynqtt458tegdy6ewxc
  text: |-
    Adversarial double-check (re-run after fixing its Finding 1) returned PASS: embed-failure escalation closes the model-loaded-but-call-fails footgun; the `take_failed` interior-mutability call coexists with `seam`'s shared `&real` borrow (two shared borrows + Cell = correct, no aliasing UB); `ExpectEmbedder` (Cell<bool>) never crosses the spawn_blocking boundary (only Arc<dyn TextEmbedder> + Handle do, both Send+Sync); check_op's conservative over-escalation is safe-side (can only over-escalate, never silently approve); the mock regression test is a genuine RED->GREEN that runs without a GPU. No further findings.

    Final state: task green in `doing`, ready for /review. Verification: nextest selection 214 passed (incl. GPU e2e that actually ran + 5 guard/regression tests); clippy -D warnings clean; cargo check --workspace clean; cargo fmt applied.
  timestamp: 2026-06-27T19:46:53.013134+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffff8980
project: expect
title: Wire a real pinned TextEmbedder + Grader panel into the expect tool layer
---
## What
The engine-level tiered compile/freeze/compare_tiered path is real and fully tested with stub seams (done in 01KW4ZD3JCRD8RR4HCG3B18DH1 / ^3b18dh1). The tool layer (`crates/swissarmyhammer-tools/src/mcp/tools/expect/mod.rs`) currently threads a **placeholder** grading seam through `approve_op`/`expectations_list`/`check_op`:
- `PlaceholderEmbedder` returns an empty (cosine-safe) vector and `tracing::warn!`s on use.
- `placeholder_judgment()` is an empty grader panel.

## Why this matters (silent false-negative footgun)
After ^3b18dh1, `expect approve` on a RESIDUAL criterion mints a golden carrying frozen Tier 2/3 assertions. With the placeholder embedder, the tool-layer compare scores BOTH golden and received sides at cosine 0 (empty vectors), so `graded_drift` sees pass==pass and reports NO drift — i.e. `expect check`/`expect list`/`approve` status can read **Approved/Passed even when the Tier 2/3 evidence genuinely diverged**. The only current signal is a `tracing::warn!`. This is a real shipping risk: a tiered golden is NOT faithfully graded in the tool layer until this lands.

## Acceptance
- Adapt the platform embedder (the `EmbedderFactory`/`TextEmbedder` that `review` loads via `default_embedder_factory`) to `swissarmyhammer_expect::TextEmbedder`, pinned to the golden's `GradingPins.embedder`.
- Build a real Tier-3 `Grader` panel from the repo `[model]` config (default + panel), resolved like `review`/`rules`, with the driving agent excluded.
- Replace `PlaceholderEmbedder`/`placeholder_judgment()` at the three call sites with the real seam; drop the warn.
- Until the real seam lands, consider a loud-and-safe interim guard: when a golden carries non-empty `tolerance`/`judgment` and only the placeholder seam is available, the tool-layer compare should surface a non-approved "uncheckable/escalate" status rather than silently reading Approved.
- Tests: a tiered golden (a tolerance + a judgment criterion) graded end-to-end through the tool layer detects drift faithfully (may gate on GPU/model availability like other model-backed tests).