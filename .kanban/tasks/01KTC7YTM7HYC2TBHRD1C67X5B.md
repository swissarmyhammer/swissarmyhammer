---
assignees:
- claude-code
depends_on:
- 01KTC7Y50PEM427HQ79NW52WY4
position_column: todo
position_ordinal: 8a80
project: semantic-search
title: BM25, trigram-Dice, and RRF scoring primitives for search fusion
---
## What
Add the three pure scoring primitives the fused `search code` op composes. New module `crates/swissarmyhammer-code-context/src/ops/search_fusion/score.rs` (sibling of `tokenize.rs` from the tokenizer card). All functions are pure — no DB, no embeddings — and consume `super::tokenize`.

Primitives (encode the agreed math exactly; constants must be named):
- BM25 (Okapi), two halves so the op can do its single corpus pass then per-chunk scoring:
  - `pub struct Bm25Corpus { /* df per query term, N (doc count), avgdl (mean token count) */ }` plus a builder that, given the query tokens and an iterator of per-document token counts + which query terms each doc contains, computes `df(t)` and `avgdl`. Keep the public shape minimal but testable.
  - `pub fn bm25_score(corpus: &Bm25Corpus, doc_tokens: &[String], query_tokens: &[String]) -> f32` implementing `Σ_t IDF(t)·tf·(k1+1)/(tf + k1·(1−b+b·|D|/avgdl))` with `const K1: f32 = 1.2; const B: f32 = 0.75;` and `IDF(t)=ln(1+(N−df+0.5)/(df+0.5))`. `tf` is term frequency of `t` in `doc_tokens`; `|D|` is `doc_tokens.len()`.
- Trigram-Dice overlap:
  - `pub fn trigram_dice(query: &str, target: &str) -> f32` = `2·|Aâ©B| / (|A|+|B|)` over char-trigram SETS (use `char_trigrams` from the tokenizer card; dedupe to sets). Returns 0.0 when either side has no trigrams. NOTE: the op will call this against the chunk's `symbol_path` + identifier tokens joined, NOT the whole chunk text — that scoping lives in the op card, this primitive just scores two strings.
- Reciprocal Rank Fusion:
  - `pub fn rrf_fuse(ranked_lists: &[&[usize]], weights: &[f32], k: f32) -> HashMap<usize, f32>` (or a Vec indexed by doc id) implementing `RRF(d)=Σ_r w_r/(k+rank_r(d))`, `const RRF_K: f32 = 60.0` as the default `k`. Each input list is doc ids in rank order (rank 0 = best). A doc absent from a list contributes nothing for that list. Document the rank base (0 vs 1) and use it consistently.

## Acceptance Criteria
- [ ] `bm25_score` matches a hand-computed Okapi value (within 1e-4) for a tiny 3-doc corpus with a 1-term and a 2-term query; rarer terms (lower df) score higher.
- [ ] `trigram_dice("get_user", "get_user")` == 1.0; `trigram_dice("getUsr", "get_user")` is high (> 0.4) proving the typo-rescue; disjoint strings score 0.0.
- [ ] `rrf_fuse` ranks a doc that is rank-0 in two of three lists above a doc that is rank-0 in only one; equal weights `[1.0,1.0,1.0]` and `k=60` reproduce a hand-computed fused ordering.
- [ ] `K1`, `B`, `RRF_K` are named `const`s, not inline literals.

## Tests
- [ ] Unit tests in `crates/swissarmyhammer-code-context/src/ops/search_fusion/score.rs` `#[cfg(test)] mod tests`: BM25 IDF/tf math vs hand-computed values, df-monotonicity (rarer term scores higher), Dice identity/typo/disjoint cases, RRF fusion ordering + weight effect + missing-doc handling.
- [ ] `cargo test -p swissarmyhammer-code-context search_fusion` passes (all new tests green).

## Workflow
- Use `/tdd` — write failing tests first, then implement to pass.