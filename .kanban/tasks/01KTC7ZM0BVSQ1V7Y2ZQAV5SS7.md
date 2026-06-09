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
- The canonical blob helpers `serialize_embedding`/`deserialize_embedding` now live in `swissarmyhammer-search` (scoring card). Code-context's inline copies in `search_code.rs` are removed.
- Cosine: `search_code` no longer needs its `pub use model_embedding::cosine_similarity;` — the crate's `search()` computes cosine internally (scalar, inline). NOTE: the leaf crate deliberately does NOT depend on `model_embedding` (it pulls `tokio` + `simsimd` + `async-trait`); the small scalar-cosine duplication is intentional.
- Keep `load_embedded_chunks` and the `EmbeddingRow` struct as the DB-loading layer.

### MUST NOT break existing consumers (verified — these compile against today's public paths)
- `swissarmyhammer_code_context::serialize_embedding` MUST remain a valid public path. The LIVE production indexer calls it at `crates/swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs:2123` (`.map(swissarmyhammer_code_context::serialize_embedding)`), and the CLI doctor imports it at `apps/code-context-cli/src/commands/doctor.rs:456`. Achieve this by RE-EXPORTING the `swissarmyhammer-search` helper from code-context's `lib.rs` crate root (keep the same name/signature). Breaking this silently breaks indexing.
- `crates/swissarmyhammer-code-context/src/ops/find_duplicates.rs` imports `cosine_similarity` from `crate::ops::search_code` (line 11) and `serialize_embedding` from it in tests (line 226). Repoint these: `cosine_similarity` → `model_embedding::cosine_similarity` directly (find_duplicates' crate already deps `model_embedding`); `serialize_embedding` → the crate-root re-export. `find_duplicates` must still compile and pass its tests.

Algorithm (build Docs, then call the crate — do NOT redesign):
- `load_embedded_chunks` already loads each chunk's `text`, `symbol_path`, `embedding`. For each row build a `Doc { id: <stable chunk id, e.g. "{file_path}:{start_line}-{end_line}">, fields: vec![ Field { weight: <high>, text: symbol_path.unwrap_or_default() }, Field { weight: <low>, text: row.text } ], embedding: Some(row.embedding) }`. Keep a side map from `Doc.id` back to the `EmbeddingRow` (or a parallel `Vec`/index map) so hits map to `SearchCodeMatch` (which needs `file_path`, `start_line`, `end_line`, `symbol_path`, `text`). Avoid gratuitously cloning the embedding twice — move it into the `Doc` where practical.
- Build `Query { text: <original query string — search_code currently receives only a query EMBEDDING; thread the query string through (new param)>, embedding: Some(query_embedding.to_vec()), weights: SignalWeights { w_bm25, w_trigram, w_cosine }, top_k, min_score: min_fused_score }`. `min_score` now filters on the NORMALIZED [0,1] fused score (per the scoring card).
- Call `swissarmyhammer_search::search(&docs, &query)`; map each `Hit` back to a `SearchCodeMatch` via the side map.

Note on trigram for code: per the scoring card, the trigram signal is a weighted sum of `trigram_dice(q.text, field.text)` across fields. Because the `text` field is large, its Dice is naturally tiny (big denominator) — so the trigram typo-rescue for code effectively comes from the short, high-weight `symbol_path` field. This is intended; no special-casing needed.

API shape changes (update `crates/swissarmyhammer-code-context/src/lib.rs` re-exports):
- `SearchCodeOptions`: REMOVE `min_similarity`. ADD `w_bm25: f32`, `w_trigram: f32`, `w_cosine: f32` (default all `1.0`) and `min_fused_score: Option<f32>` (`None` = unfiltered). Keep `top_k`, `language`, `file_pattern`. Update `Default`.
- `search_code` fn signature: add the query string param (e.g. `query_text: &str`) alongside the existing `query_embedding: &[f32]`.
- `SearchCodeMatch`: REPLACE `similarity: f32` with `score: f32` (fused, normalized) plus `signals: Signals { bm25, trigram, cosine }` — reuse `swissarmyhammer_search::Signals` and re-export it from code-context. Derive `Debug, Clone, Serialize`.
- `SearchCodeResult` and `IndexingProgress`/`compute_indexing_progress` unchanged.
- `lib.rs` must keep `serialize_embedding` exported (re-export from search crate) and export the `Signals` type.
- Migrate the in-file `#[cfg(test)] mod tests`: fixture tests (`test_search_code_ranking`, `test_search_code_top_k`, `test_search_code_min_similarity_filter`, etc.) reference `min_similarity`/`.similarity` and MUST move to the new shape (assert on `score`/`signals`; convert the cosine-floor test to a `min_fused_score` test). The cosine-contract unit tests (`test_cosine_similarity_*`) and the blob round-trip test move to `swissarmyhammer-search` (scoring card) — delete them here.

### Performance note (known, accepted — not a blocker)
The old path only cosined each chunk; the new path ALSO tokenizes every chunk's field text on every query (the stateless crate can't cache it). This is the real cost of fusion and is accepted under the loop-and-cosine premise. If large-repo query latency later becomes a problem, a FOLLOW-UP card (not this one) can cache per-chunk token lists keyed by chunk id, or tokenize at index time. Do not build that here.

## Acceptance Criteria
- [ ] `SearchCodeOptions` has `w_bm25`/`w_trigram`/`w_cosine` (default 1.0) and `min_fused_score: Option<f32>`; `min_similarity` is gone. `cargo build -p swissarmyhammer-code-context` compiles.
- [ ] `search_code` accepts the query string and builds `Doc`s (symbol_path high weight, text low weight, embedding) and calls `swissarmyhammer_search::search`.
- [ ] `SearchCodeMatch` exposes `score` (normalized) and `signals { bm25, trigram, cosine }`; `similarity` field removed.
- [ ] `swissarmyhammer_code_context::serialize_embedding` is still a valid public path (re-exported from `swissarmyhammer-search`); `cargo build -p swissarmyhammer-tools` (the live indexer at mod.rs:2123) and `cargo build -p code-context-cli` (doctor.rs:456) still compile.
- [ ] `find_duplicates.rs` compiles with repointed imports and its tests pass (`cargo test -p swissarmyhammer-code-context find_duplicates`).
- [ ] An exact-identifier query whose cosine is low but whose symbol_path matches still ranks via the bm25/trigram contribution (unit test below).
- [ ] `lib.rs` re-exports compile with the new/renamed public types (`Signals` exported, `serialize_embedding` re-exported).

## Tests
- [ ] Migrate + extend `#[cfg(test)] mod tests` in `search_code.rs`: a fusion-ranking test where a chunk with a strong symbol_path/bm25 match but weak cosine out-ranks a chunk with mediocre signals; a `min_fused_score` floor test (on normalized score); a `signals` breakdown presence test; a weights-affect-ordering test (boosting `w_cosine` reorders results). Keep the `IndexingProgress` tests as-is.
- [ ] `cargo test -p swissarmyhammer-code-context search_code` passes (migrated + new tests green), and `find_duplicates` tests still pass.

## Workflow
- Use `/tdd` — write failing tests first, then implement to pass.