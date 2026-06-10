---
assignees:
- claude-code
position_column: review
position_ordinal: '8480'
title: 'Embedding: process-wide one-inference-at-a-time semaphore gate'
---
## Goal (generalizes the review-OOM fix)
Limit embedding inference to ONE per process via a semaphore, so every caller (review probes, code-context indexer, entity search) queues rather than running concurrent model inferences. The per-review-pipeline cap only covered review; this protects all embedding regardless of caller or `Embedder` instance count.

## Implementation
`crates/swissarmyhammer-embedding/src/embedder.rs`:
- `static EMBEDDING_GATE: Semaphore = Semaphore::const_new(1)` — process-global.
- `async fn with_embedding_permit(op)` — acquires the permit, runs `op`; factored out so it's unit-testable without a loaded model.
- `Embedder::embed_text` wraps its whole body (single + chunk-pool path) in `with_embedding_permit`, so one logical embedding finishes before the next starts. Backends keep their own internal mutex; no re-entrancy (the gate is only acquired at the `Embedder` dispatch layer).
- Added `tokio` (feature `sync`) to `[dependencies]` (was dev-only).

All production embedding funnels through `swissarmyhammer_embedding::Embedder` (verified: review_op, treesitter index, entity-search), so this is the universal chokepoint.

## Verified
- `with_embedding_permit_runs_one_at_a_time` (3 racing tasks → peak concurrency 1), 0.18s.
- swissarmyhammer-embedding 37/37; full-workspace `clippy -D warnings` clean.

#review #memory #embedding