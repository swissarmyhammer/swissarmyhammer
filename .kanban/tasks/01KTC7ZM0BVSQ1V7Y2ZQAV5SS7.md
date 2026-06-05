---
assignees:
- claude-code
depends_on:
- 01KTC7Y50PEM427HQ79NW52WY4
- 01KTC7YTM7HYC2TBHRD1C67X5B
position_column: todo
position_ordinal: 8b80
project: semantic-search
title: Rewrite search_code to build Docs and call swissarmyhammer-search::search
---
## What
Rewrite the embedding-only `search_code` in `crates/swissarmyhammer-code-context/src/ops/search_code.rs` to build `swissarmyhammer_search::Doc`s from the already-loaded `ts_chunks` rows and rank them by calling `swissarmyhammer_search::search`. The BM25/trigram/RRF/cosine logic now lives in the `swissarmyhammer-search` crate — do NOT create a code-context-local `search_fusion` module.

Dependency wiring:
- `crates/swissarmyhammer-code-context/Cargo.toml`: add `swissarmyhammer-search = { workspace = true }`.
- The crate's inline `serialize_embedding`/`deserialize_embedding` and the `pub use model_embedding::cosine_similarity` in `search_code.rs` are superseded by the crate's helpers. Either re-export from `swissarmyhammer-search` or keep `deserialize_embedding` local for blob loading and drop the cosine re-export (the crate's `search()` computes cosine internally). Keep `load_embedded_chunks` and the `EmbeddingRow` struct as the DB-loading layer.

Algorithm (build Docs, then call the crate — do NOT redesign):
- `load_embedded_chunks` already loads each chunk's `text`, `symbol_path`, `embedding`. For each row build a `Doc { id: <stable chunk id, e.g. "{file_path}:{start_line}-{end_line}">, fields: vec![ Field { weight: <high>, text: symbol_path.unwrap_or_default() }, Field { weight: <low>, text: row.text } ], embedding: Some(row.embedding) }`. Keep a side map from `Doc.id` back to the `EmbeddingRow` so hits can be mapped to `SearchCodeMatch`.
- Build `Query { text: <the original query string — NOTE: search_code currently receives only a query EMBEDDING, not text; thread the query string through>, embedding: Some(query_embedding.to_vec()), weights: SignalWeights { w_bm25, w_trigram, w_cosine }, top_k, min_score: min_fused_score }`. This means `search_code`'s signature must also accept the query string (the MCP wrapper card already embeds text and has the string available — pass both).
- Call `swissarmyhammer_search::search(&docs, &query)`; map each `Hit` back to a `SearchCodeMatch` via the side map.

API shape changes (update `crates/swissarmyhammer-code-context/src/lib.rs` re-exports if names change):
- `SearchCodeOptions`: REMOVE `min_similarity`. ADD `w_bm25: f32`, `w_trigram: f32`, `w_cosine: f32` (default all `1.0`) and `min_fused_score: Option<f32>` (`None` = unfiltered). Keep `top_k`, `language`, `file_pattern`. Update `Default`.
- `search_code` fn signature: add the query string param (e.g. `query_text: &str`) alongside the existing `query_embedding: &[f32]`.
- `SearchCodeMatch`: REPLACE `similarity: f32` with `score: f32` (fused) plus `signals: SearchSignals { bm25: f32, trigram: f32, cosine: f32 }` — or reuse `swissarmyhammer_search::Signals` directly and re-export it. Derive `Debug, Clone, Serialize`.
- `SearchCodeResult` and `IndexingProgress`/`compute_indexing_progress` unchanged.
- Migrate the in-file `#[cfg(test)] mod tests`: the fixture tests (`test_search_code_ranking`, `test_search_code_top_k`, `test_search_code_min_similarity_filter`, etc.) reference `min_similarity`/`.similarity` and MUST move to the new shape (assert on `score`/`signals`; convert the cosine-floor test to a `min_fused_score` test or drop it). The cosine-contract unit tests (`test_cosine_similarity_*`) and the blob round-trip test move to the `swissarmyhammer-search` crate (scoring card) — delete or thin them here.

Note: equal weights are the default but tunable without code changes. The `0.7` cosine floor is GONE (replaced by the optional fused-score floor).

## Acceptance Criteria
- [ ] `SearchCodeOptions` has `w_bm25`/`w_trigram`/`w_cosine` (default 1.0) and `min_fused_score: Option<f32>`; `min_similarity` is gone. `cargo build -p swissarmyhammer-code-context` compiles.
- [ ] `search_code` accepts the query string and builds `Doc`s (symbol_path high weight, text low weight, embedding) and calls `swissarmyhammer_search::search`.
- [ ] `SearchCodeMatch` exposes `score` and `signals { bm25, trigram, cosine }`; `similarity` field removed.
- [ ] An exact-identifier query whose cosine is low but whose symbol_path matches still ranks via the bm25/trigram contribution (covered by unit test below).
- [ ] `lib.rs` re-exports compile with the new/renamed public types (signals type exported).

## Tests
- [ ] Migrate + extend `#[cfg(test)] mod tests` in `search_code.rs`: a fusion-ranking test where a chunk with a strong symbol_path/bm25 match but weak cosine out-ranks a chunk with mediocre signals; a `min_fused_score` floor test; a `signals` breakdown presence test; a weights-affect-ordering test (boosting `w_cosine` reorders results). Keep the `IndexingProgress` tests as-is.
- [ ] `cargo test -p swissarmyhammer-code-context search_code` passes (migrated + new tests green).

## Workflow
- Use `/tdd` — write failing tests first, then implement to pass.