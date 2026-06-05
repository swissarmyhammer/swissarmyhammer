---
assignees:
- claude-code
depends_on:
- 01KTC7Y50PEM427HQ79NW52WY4
- 01KTC7YTM7HYC2TBHRD1C67X5B
position_column: todo
position_ordinal: 8b80
project: semantic-search
title: Rewrite search_code op into single-pass three-signal rank fusion
---
## What
Rewrite the embedding-only `search_code` in `crates/swissarmyhammer-code-context/src/ops/search_code.rs` into a three-signal fused search computed in ONE Rust pass over the already-loaded `ts_chunks` rows. NO FTS5, no second index, no ANN. `load_embedded_chunks` already loads each chunk's `text`, `symbol_path`, and `embedding`; BM25 and trigram piggyback on that same in-memory `Vec<EmbeddingRow>`.

Algorithm (agreed — do NOT redesign):
- Tokenize the query once via `super::search_fusion::tokenize::tokenize`.
- Pass 1 (corpus stats over all rows): build the BM25 corpus — `df(t)` per query term and `avgdl` (mean token count over all rows). Use `Bm25Corpus` from the scoring-primitives card.
- Pass 2 (per-chunk signals):
  - `cosine` — existing `cosine_similarity(query_embedding, &row.embedding)`.
  - `bm25` — `bm25_score(&corpus, &doc_tokens, &query_tokens)` where `doc_tokens = tokenize(&row.text)`.
  - `trigram` — `trigram_dice(query_str, &scope)` where `scope` is the chunk's `symbol_path` (if any) joined with its identifier tokens (tokens of `symbol_path` + the chunk's identifier-like tokens), NOT the whole chunk text. Document the exact scope string construction.
- Fuse via RRF: rank all chunks within each of the three signal lists (sort indices by signal desc), then `rrf_fuse(&[&bm25_ranks, &trigram_ranks, &cosine_ranks], &[w_bm25, w_trigram, w_cosine], RRF_K)`. Sort by fused score desc, take `top_k`.

API shape changes (update `crates/swissarmyhammer-code-context/src/lib.rs` re-exports if names change):
- `SearchCodeOptions`: REMOVE `min_similarity`. ADD `w_bm25: f32`, `w_trigram: f32`, `w_cosine: f32` (default all `1.0`) and `min_fused_score: Option<f32>` (optional fused-score floor; `None` = return top_k unfiltered). Keep `top_k`, `language`, `file_pattern`. Update `Default`.
- `SearchCodeMatch`: REPLACE `similarity: f32` with `score: f32` (fused) plus `signals: SearchSignals { bm25: f32, trigram: f32, cosine: f32 }` (new struct, derive `Debug, Clone, Serialize`). All three signals are already computed — keep them for debugging rank order.
- `SearchCodeResult` and `IndexingProgress` / `compute_indexing_progress` are unchanged.
- Update the in-file `#[cfg(test)] mod tests`: the existing fixture tests (`test_search_code_ranking`, `test_search_code_top_k`, `test_search_code_min_similarity_filter`, etc.) reference `min_similarity` and `.similarity` and MUST be migrated to the new shape (assert on `score`/`signals`, drop the cosine-floor filter test or convert it to a `min_fused_score` test).

Note: equal weights are the default but exposed as options so they are tunable without code changes. Drop the `0.7` cosine floor entirely (replaced by optional fused-score floor).

## Acceptance Criteria
- [ ] `SearchCodeOptions` has `w_bm25`/`w_trigram`/`w_cosine` (default 1.0) and `min_fused_score: Option<f32>`; `min_similarity` is gone. `cargo build -p swissarmyhammer-code-context` compiles.
- [ ] `SearchCodeMatch` exposes `score` and `signals { bm25, trigram, cosine }`; `similarity` field removed.
- [ ] Corpus stats (`df`, `avgdl`) computed once in pass 1; signals computed once per chunk in pass 2 — no re-loading of rows.
- [ ] An exact-identifier query whose embedding cosine is low but whose symbol_path matches still ranks via the bm25/trigram contribution (covered by unit test below).
- [ ] `lib.rs` re-exports compile with the new/renamed public types (`SearchSignals` exported).

## Tests
- [ ] Migrate + extend `#[cfg(test)] mod tests` in `search_code.rs`: a fusion-ranking test where chunk with a strong symbol_path/bm25 match but weak cosine out-ranks a chunk with mediocre signals; a `min_fused_score` floor test; a `signals` breakdown presence test; weights-affect-ordering test (boosting `w_cosine` reorders results).
- [ ] `cargo test -p swissarmyhammer-code-context search_code` passes (migrated + new tests green).

## Workflow
- Use `/tdd` — write failing tests first, then implement to pass.