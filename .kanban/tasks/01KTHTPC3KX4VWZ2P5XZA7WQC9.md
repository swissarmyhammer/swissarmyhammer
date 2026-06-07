---
assignees:
- claude-code
position_column: review
position_ordinal: '8280'
title: 'Review OOM: share the embedder process-wide (singleton)'
---
## Problem
`review_op::default_embedder_factory` constructs and `.load()`s a fresh `Embedder` (the qwen-embedding model, ~hundreds-of-MB to GB) on EVERY review run. Nothing caches it process-wide, so each run pays the load and holds its own copy.

## Fix
Cache the loaded default embedder in a process-global `tokio::sync::OnceCell<Arc<dyn TextEmbedder>>` (keyed by the default model name). `default_embedder_factory` returns the cached `Arc` on subsequent runs. Safe because review runs are serialized (see the cap=1 task) and embedding within a run is sequential.

Keep the injection seam intact: tests inject the mock embedder factory, which must NOT hit the cache.

#review #bug #memory