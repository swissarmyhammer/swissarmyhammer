---
assignees:
- claude-code
position_column: todo
position_ordinal: ab80
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