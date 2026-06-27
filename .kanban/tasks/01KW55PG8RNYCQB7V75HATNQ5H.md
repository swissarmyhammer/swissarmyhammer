---
assignees:
- claude-code
position_column: todo
position_ordinal: a380
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