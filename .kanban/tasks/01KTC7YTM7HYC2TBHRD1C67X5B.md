---
assignees:
- claude-code
depends_on:
- 01KTC7Y50PEM427HQ79NW52WY4
position_column: todo
position_ordinal: 8a80
project: semantic-search
title: 'swissarmyhammer-search: BM25 + trigram-Dice + RRF + cosine + search()'
---
## What
Implement the scoring primitives and the top-level `search()` in the new `swissarmyhammer-search` crate (created in the types+tokenizer card). All pure, in-memory, NO DB, NO FTS5, NO ANN, NO persistence.

Files to create/edit:
- `crates/swissarmyhammer-search/src/score.rs` — BM25, trigram-Dice, RRF primitives (consume `crate::tokenize`).
- `crates/swissarmyhammer-search/src/cosine.rs` — inlined `cosine_similarity` + little-endian f32 blob helpers (do NOT depend on `model_embedding`).
- `crates/swissarmyhammer-search/src/lib.rs` — add `pub fn search(docs: &[Doc], q: &Query) -> Vec<Hit>` (the two-pass loop) and re-export the helpers.

### Field weighting model (DECIDED — weighted-tf BM25F-lite)
A doc has multiple `Field`s with weights. Combine them as **weighted term frequency under a single global IDF** (NOT per-field BM25 summed, NOT full BM25F with per-field length norm):
- One IDF per term, computed over the corpus where `df(t)` = number of docs containing `t` in ANY field.
- When scoring a doc, `tf(t, doc)` = sum of `Field.weight` over each occurrence of `t` across all the doc's fields (a term appearing once in a weight-3.0 title contributes `tf += 3.0`; once in a weight-1.0 body contributes `tf += 1.0`).
- `|D|` (length normalization) = the doc's UNWEIGHTED total token count; `avgdl` = mean unweighted token count across all docs.
This makes title/symbol_path matches naturally outrank body matches with a single IDF and one pass, and is hand-computable for tests.

Scoring primitives (named constants, exact math):
- BM25 (Okapi), two halves so `search()` does one corpus pass then per-doc scoring:
  - `pub struct Bm25Corpus { /* df per query term, N (doc count), avgdl (mean UNWEIGHTED token count) */ }` + a builder taking query tokens and an iterator of per-doc unweighted token counts + which query terms each doc contains (in any field); computes `df(t)` and `avgdl`.
  - `pub fn bm25_score(corpus: &Bm25Corpus, weighted_tf: &HashMap<&str, f32>, doc_len: usize, query_tokens: &[String]) -> f32` implementing `Σ_t IDF(t)·tf·(k1+1)/(tf + k1·(1−b+b·|D|/avgdl))` with `const K1: f32 = 1.2; const B: f32 = 0.75;` and `IDF(t)=ln(1+(N−df+0.5)/(df+0.5))`. `tf` = the weighted term frequency described above; `|D|` = `doc_len` (unweighted).
- Trigram-Dice: `pub fn trigram_dice(query: &str, target: &str) -> f32` = `2·|A∩B| / (|A|+|B|)` over char-trigram SETS (use `char_trigrams`; dedupe to sets). Returns 0.0 when either side has no trigrams. The per-doc trigram SIGNAL (in `search()`) is the weighted sum across the doc's fields: `Σ_field weight_field · trigram_dice(q.text, field.text)` — no magic "high-weight" threshold; high-weight fields dominate. (Absolute scale is irrelevant: RRF consumes only the rank.)
- RRF: `pub fn rrf_fuse(ranked_lists: &[&[usize]], weights: &[f32], k: f32) -> HashMap<usize, f32>` implementing `RRF(d)=Σ_r w_r/(k+rank_r(d))`, `const RRF_K: f32 = 60.0` default `k`. Each input list is doc indices in rank order (rank 0 = best). A doc absent from a list contributes nothing for that list. Ranking within a signal uses a STABLE sort (ties keep input/doc-index order) so fused rankings are deterministic in tests. Document the rank base (0).

### Fused-score normalization (DECIDED — surface an interpretable score)
Raw RRF scores are tiny/unintuitive (three signals at rank 0, k=60 → ≈0.05). Normalize the fused score to [0,1] by dividing by the max achievable for the PRESENT signals: `max = Σ_{present r} w_r / (k + 0)`. This is monotone (ranking unchanged) but makes the surfaced `Hit.score` and the `Query.min_score` floor meaningful. `min_score` is applied to this NORMALIZED score.

Cosine + blob helpers (`cosine.rs`):
- `pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32` — dot/norm; returns 0.0 for empty or mismatched-length inputs (match the semantics the old `model_embedding::cosine_similarity` had — identical=1.0, orthogonal=0.0, opposite=-1.0, empty=0.0, mismatched=0.0).
- `pub fn serialize_embedding(embedding: &[f32]) -> Vec<u8>` (little-endian f32 blob) and `pub fn deserialize_embedding(blob: &[u8]) -> Vec<f32>` (mirror the helpers currently inline in `code-context/src/ops/search_code.rs`; callers persist vectors).

Top-level `search()` (two passes over the in-memory `&[Doc]`):
- Tokenize each doc's fields ONCE (reuse across stats + scoring — do not re-tokenize per pass). Tokenize `q.text` once.
- Pass 1: build `Bm25Corpus` (df per query term over docs containing the term in any field; avgdl over unweighted doc token counts).
- Pass 2 per doc: compute `bm25` (weighted-tf BM25F-lite above); `trigram` (weighted-sum Dice above); `cosine` over `q.embedding` vs `doc.embedding` when BOTH are present.
- Fuse: rank docs within each PRESENT signal list, `rrf_fuse` with `q.weights`. A signal with no data (e.g. all `embedding: None`, or `q.embedding: None`, or empty `q.text`) is simply absent from fusion — graceful degradation, not a zero-fill. Normalize fused score to [0,1] (above). Sort by fused score desc, apply `q.min_score` floor if set, take `q.top_k`. Return `Vec<Hit>` carrying the per-signal `Signals` (raw bm25/trigram/cosine values, for debugging rank order).

## Acceptance Criteria
- [ ] `bm25_score` matches a hand-computed Okapi value (within 1e-4) for a tiny 3-doc corpus with a 1-term and a 2-term query; rarer terms (lower df) score higher.
- [ ] Weighted-tf field weighting: a query term present once in a high-weight field scores higher than the same term once in a low-weight field, all else equal (hand-computed).
- [ ] `trigram_dice("get_user","get_user")` == 1.0; `trigram_dice("getUsr","get_user")` > 0.4 (typo-rescue); disjoint strings score 0.0.
- [ ] `rrf_fuse` ranks a doc rank-0 in two of three lists above a doc rank-0 in only one; equal weights `[1,1,1]`, `k=60` reproduce a hand-computed fused ordering; ties resolve by input order (stable).
- [ ] Fused score is normalized to [0,1]: a doc ranked rank-0 in every present signal scores 1.0; normalization does not change ordering.
- [ ] `cosine_similarity` satisfies identical=1.0, orthogonal=0.0, opposite=-1.0, empty=0.0, mismatched-length=0.0.
- [ ] `serialize_embedding`/`deserialize_embedding` round-trip a `Vec<f32>` exactly.
- [ ] `search()` ranks a doc with a strong high-weight-field lexical match but weak cosine above a doc with mediocre signals; when no doc has an embedding the cosine signal is absent (not zero-filled) and results still come back; `min_score` filters on the normalized score.
- [ ] `K1`, `B`, `RRF_K` are named `const`s, not inline literals.

## Tests
- [ ] Unit tests in `score.rs`, `cosine.rs`, and a `search()` test module: BM25 IDF/tf vs hand-computed values + df-monotonicity + weighted-tf field effect; Dice identity/typo/disjoint; RRF ordering + weight effect + stable ties + missing-doc handling; fused-score normalization (rank-0-everywhere == 1.0, ordering preserved); cosine contract cases + blob round-trip; `search()` fusion-ordering + graceful-degradation (embedding: None / empty query) + min_score-on-normalized.
- [ ] `cargo test -p swissarmyhammer-search` passes (all new tests green).

## Workflow
- Use `/tdd` — write failing tests first, then implement to pass.