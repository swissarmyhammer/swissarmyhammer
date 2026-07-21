---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01ky1w2tbc7wtrrsh8z5a5twqg
  text: 'Last card in the #review batch. Iteration 1: /implement. This is the pre-existing embedder.rs/retry.rs debt split out of ^sqmq62v''s round-5 churn (trait docs, LLAMA_DEFAULT_MAX_SEQUENCE_LENGTH const, advance_to_char_boundary helper, classify_hf_error extraction). Line numbers in the card are from the round-5 review of commit 61c9ea5 — but several later commits have touched embedder.rs/retry.rs since (^1tt5pa6 added DownloadingModel handling), so LOCATE BY SYMBOL not line number.'
  timestamp: 2026-07-21T08:18:38.572507+00:00
- actor: claude-code
  id: 01ky1wnxbq4eyp6rft2wrkbabp
  text: |-
    Implemented all four items (TDD for the two new helpers, RED watched for both). Located everything by symbol per the stale-line-number warning.

    1. Trait docs: added /// docs to the four methods in `impl TextEmbedder for Embedder` in embedder.rs (load, embed_text, embedding_dimension, is_loaded), with # Errors sections on the two Result-returning ones to match the crate's existing convention. NOTE: the TextEmbedder trait *definition* lives in model-embedding/src/lib.rs and was already fully documented; the card's line numbers (~303-337) map to the impl block, which is what lacked docs.
    2. Const: added `const LLAMA_DEFAULT_MAX_SEQUENCE_LENGTH: usize = 512;` beside ANE_DEFAULT_MAX_SEQUENCE_LENGTH (NOT macos-gated, since the Llama path is cross-platform) and used it at the Llama backend construction default (was `unwrap_or(512)`).
    3. Extraction: added `fn advance_to_char_boundary(s: &str, pos: usize) -> usize` (hand-rolled; str::floor/ceil_char_boundary are still unstable on edition 2021, and the needed semantics is ceil/forward, not floor). Used it to replace the THREE `while X < text.len() && !is_char_boundary(X)` loops (search_end, actual_end, start) in chunk_text; the actual_end `if/else` collapsed to a single call. Intentionally LEFT the fourth loop (search_start) unchanged: it is bounded by `end`, not `text.len()`, and the `chunk_text_unicode_boundaries` test (chunk_size=15 -> chunk_size/10=1, tiny search window) exercises exactly the path where the `end` cap vs `s.len()` cap would diverge, so replacing it could change edge-case behavior. New unit test advance_to_char_boundary_moves_past_mid_char_position (héllo, byte 2 mid-char -> 3).
    4. Extraction: added `fn classify_hf_error(error_msg: &str) -> ModelError` in retry.rs from the retry closure's Err arm; closure body now `Err(classify_hf_error(&e.to_string()))` (was ~5 levels deep). Kept retry.rs's exact behavior (lowercase matching + lowercase "huggingface..." messages) rather than reusing the existing `From<ApiError>` impl in error.rs, which matches case-sensitively and emits "HuggingFace..." (capital) — reuse would have changed behavior. New unit test classify_hf_error_maps_messages_to_variants.

    Gates all green: `cargo nextest run -p model-loader -p swissarmyhammer-embedding -p llama-embedding` = 281 passed 0 failed; both new tests pass; `cargo fmt` clean; `cargo clippy -p model-loader -p swissarmyhammer-embedding -p llama-embedding --all-targets -- -D warnings` clean; `cargo check --workspace --all-targets` clean. Diff scoped to retry.rs + embedder.rs only. Leaving in doing for review.
  timestamp: 2026-07-21T08:29:04.247420+00:00
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffffff780
title: 'Cleanup: pre-existing quality debt in embedder.rs / retry.rs (trait docs, 512 const, nesting)'
---
## What

Pre-existing code-quality debt surfaced by the review engine during ^sqmq62v's round-5 review but sitting OUTSIDE that commit's diff hunks (verified via `git diff HEAD~1..HEAD --unified=0`: the delta touched only `embedder.rs` ~215-217/~508-523 and `retry.rs` ~169-172/~214-236). These are genuine improvements to code that predates the download-progress work; split out here rather than expanding ^sqmq62v's diff into untouched code (which would perpetuate a review-churn loop). Fix each at root.

1. `crates/swissarmyhammer-embedding/src/embedder.rs` — add doc comments to the four public `TextEmbedder` trait methods lacking them: `load` (~:303), `embed_text` (~:311), `embedding_dimension` (~:329), `is_loaded` (~:337). Sweep the trait: document every public method so none remain bare.
2. `crates/swissarmyhammer-embedding/src/embedder.rs:~195` — the Llama backend's `max_sequence_length` default `512` is a bare literal while the ANE path uses `ANE_DEFAULT_MAX_SEQUENCE_LENGTH`. Define `const LLAMA_DEFAULT_MAX_SEQUENCE_LENGTH: usize = 512;` and use it. (Note: ^sqmq62v round 4 reasoned 512 was single-use so not "duplicated"; the rule here is the symmetry/named-constant one, not duplication — give it a name to match its ANE sibling.)
3. `crates/swissarmyhammer-embedding/src/embedder.rs:~374` — three while-loops repeat character-boundary validation; extract `fn advance_to_char_boundary(s: &str, pos: usize) -> usize` (or reuse `str::floor_char_boundary` if the MSRV allows) and call it from all three sites, flattening the level-3 nesting.
4. `crates/model-loader/src/retry.rs:~121` — the HF-error classification `if`-chain is nested 5 deep inside the retry closure (`Err` arm → match → async block → closure). Extract `fn classify_hf_error(error_msg: &str) -> ModelError` (or similar) so the closure body is 1-2 levels.

Behavior must be identical — these are doc/const/extraction refactors only. Line numbers are approximate (from the round-5 review of commit 61c9ea5); locate by symbol.

## Acceptance Criteria
- [ ] All four `TextEmbedder` trait methods (and any other bare public trait methods) carry doc comments
- [ ] `512` Llama default replaced by a named `const`, used at its site; no bare `512` literal remains for that default
- [ ] Character-boundary logic exists in exactly one helper, called from all former inline sites; the former level-3 nesting is gone
- [ ] HF-error classification extracted to a named function; the retry closure body is no longer 4+ levels deep
- [ ] `cargo clippy -p model-loader -p swissarmyhammer-embedding --all-targets -- -D warnings` clean; behavior unchanged (all existing tests still pass without modification)

## Tests
- [ ] Existing suite proves behavior preservation: `cargo nextest run -p model-loader -p swissarmyhammer-embedding -p llama-embedding` green with no test changes (pure refactor)
- [ ] Add a focused unit test for the extracted `classify_hf_error` (input error strings → expected `ModelError` variant) in `crates/model-loader/src/retry.rs`, and for `advance_to_char_boundary` (multi-byte string, position mid-char → next boundary) in `embedder.rs`
- [ ] Run: `cargo nextest run -p model-loader -p swissarmyhammer-embedding` — green, under 10s per unit test

## Workflow
- Use `/tdd` — the two new helpers get failing unit tests first; the doc/const changes are covered by the unchanged existing suite staying green. #review