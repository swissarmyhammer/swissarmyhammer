---
assignees:
- claude-code
position_column: review
position_ordinal: '80'
title: 'Review OOM: probe runner re-loads the entire embedding index per file/function'
---
## Symptom
`review` (the new local multi-agent review) OOMs on a real repo.

## Root cause
The engine probe stage materializes the **entire** `ts_chunks` embedding table into a fresh `Vec` on every call:
- `swissarmyhammer-code-context` `find_duplicates()` and `search_code()` both call `load_embedded_chunks`, which `SELECT ... embedding FROM ts_chunks` and deserializes every row into a `Vec<f32>`.
- This index is ~50,285 chunks × 1024-dim f32 = ~206MB embeddings + ~39MB text ≈ **~250MB per call**, with 50k separate 4KB heap allocs per load.

The MCP code_context tool calls each op **once** (fine). But the review's probe runner (`crates/swissarmyhammer-validators/src/review/probes.rs`) calls:
- `find_duplicates` **once per changed file** (`run_duplicates`)
- `search_code` **once per added function** (`run_similar`)

So a real review fires dozens of full ~250MB index materializations back-to-back → enormous allocator churn → OOM.

## Fix
Load the embedding corpus **once per review run** and reuse it across all duplicate/similar probes. Share the existing ranking/grouping core in code-context (no reimplementation):
- code-context: add `LoadedChunk` + `load_all_embedded_chunks(conn)`, factor the cosine ranking/grouping into corpus variants (`find_duplicates_in`, `search_loaded`); refactor public `find_duplicates`/`search_code` to delegate to them (behavior unchanged).
- probes.rs: load the corpus once in `run_probes` when duplicates/similar are requested, pass `&corpus` into `run_duplicates`/`run_similar`.

#review #bug #memory