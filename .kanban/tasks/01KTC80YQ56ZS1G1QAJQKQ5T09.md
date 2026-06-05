---
assignees:
- claude-code
depends_on:
- 01KTC808B8H9PX5DXPXZ9R1BCQ
position_column: todo
position_ordinal: 8d80
project: semantic-search
title: 'Real-pipeline e2e: exact-identifier query that cosine-only misses now ranks top via fusion'
---
## What
Add the keystone end-to-end test proving the fused `search code` rescues a query that pure-cosine misses. This MUST run through the REAL indexer -> real `search code` MCP path, following the reference pattern in `crates/swissarmyhammer-tools/tests/integration/semantic_search_e2e.rs` (real `index_discovered_files_async` -> registered MCP tool dispatch -> assert on ranking). NOT a fixture-only test (no raw-SQL-inserted embeddings).

Add a new test (new file `crates/swissarmyhammer-tools/tests/integration/search_fusion_e2e.rs`, or a new `#[tokio::test] #[serial_test::serial(cwd)]` in the existing `semantic_search_e2e.rs` reusing its helpers — prefer reusing the existing helpers `make_*` / `extract_text` / index harness):
- Build a tiny project containing a file with a distinctively-named identifier whose embedding cosine to the query string is weak but whose name is an exact/near-exact lexical match — e.g. a function `fn reticulate_splines()` plus several decoy files of ordinary code. Choose the query so that pure cosine would NOT rank the target first (the decoys are semantically closer in embedding space), but BM25/trigram on the symbol_path pushes it to #1.
- Index with the real indexer; assert `ts_chunks.embedding IS NOT NULL` and `indexed_files.embedded = 1` (as the reference test does), so we know the embedding signal is genuinely present and fusion — not an empty-cosine fallback — is doing the work.
- Dispatch `search code` with the lexical query (e.g. `"reticulate splines"` or a partial/typo like `"reticulate_splne"`).
- Assert the target file ranks `matches[0]`, and assert the response `matches[0].signals.bm25 > 0.0` (or `trigram > 0.0`) while `signals.cosine` is NOT the top cosine — proving fusion, not cosine alone, produced the rank.

ALSO: update the existing `semantic_search_e2e.rs` assertions that read `m.get("similarity")` (in the failure-message map around the `auth.rs` ranking assert) to read `score`/`signals` per the new response shape, so the existing e2e still compiles and passes.

This is the card that proves the whole feature. It is GPU/model-dependent like the existing e2e — gate it the same way the reference test is gated (it already runs the real embedder under `#[serial_test::serial(cwd)]`).

## Acceptance Criteria
- [ ] New e2e test exists, runs through the real indexer + real `search code` MCP dispatch (no raw-SQL embedding inserts).
- [ ] The target identifier file ranks `matches[0]` for a lexical/typo query that pure-cosine ranking would NOT rank first.
- [ ] The test asserts the fused match's `signals.bm25` (or `signals.trigram`) is non-zero, proving the lexical signal drove the rank.
- [ ] The existing `semantic_search_e2e.rs` is updated to the `score`/`signals` response shape and still passes.

## Tests
- [ ] `cargo test -p swissarmyhammer-tools --test integration search_fusion` (or the new test name) passes.
- [ ] `cargo test -p swissarmyhammer-tools --test integration semantic_search` still passes after the response-shape migration.

## Workflow
- Use `/tdd` — write the failing e2e first (it fails on trunk because cosine-only mis-ranks the target), then rely on the implemented fusion to make it pass.