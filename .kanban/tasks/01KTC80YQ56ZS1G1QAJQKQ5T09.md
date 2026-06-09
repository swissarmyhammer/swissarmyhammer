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
Add the keystone end-to-end test proving the fused `search code` rescues a query that pure-cosine misses. Runs through the REAL indexer and REAL embedder following the reference pattern in `crates/swissarmyhammer-tools/tests/integration/semantic_search_e2e.rs` (real `index_discovered_files_async` -> real `search_code` / registered MCP dispatch). NOT fixture-only (no raw-SQL-inserted embeddings). The embedder is always available (the Test runner has a GPU) — embed for real; there is no model-free path.

### Proof strategy: a WEIGHT DIFFERENTIAL on an ordinary corpus
Do NOT try to engineer files so the embedding model ranks them a certain way — you cannot reliably predict the model's absolute cosine ranking. Instead prove fusion changed the outcome by flipping the internal weights on the SAME indexed DB. `SearchCodeOptions` weights are public fields the integration test can set directly. This works on any small ordinary corpus; no special "adversarial" files are needed.

New test (new file `crates/swissarmyhammer-tools/tests/integration/search_fusion_e2e.rs`, or a new `#[tokio::test] #[serial_test::serial(cwd)]` in `semantic_search_e2e.rs` reusing its helpers — prefer reusing the existing `make_*`/`extract_text`/index harness):
- Build a tiny project: one file with a distinctively-named identifier, e.g. `fn reticulate_splines()`, plus a few other ordinary code files.
- Index with the REAL indexer; assert `ts_chunks.embedding IS NOT NULL` and `indexed_files.embedded = 1` (as the reference test does) so the embedding signal is genuinely present and fusion — not an empty-cosine fallback — is doing the work.
- Use a TYPO/partial query (e.g. `"reticulate_splne"`): a typo embeds to something off, so cosine-only won't rank the target first, while trigram on `symbol_path` nails it — making the differential reliable.
- Embed the query ONCE (real embedder). Call `swissarmyhammer_code_context::search_code(&db, query_text, query_embedding, &options)` TWICE against the same DB:
  - cosine-only `SearchCodeOptions { w_bm25: 0.0, w_trigram: 0.0, w_cosine: 1.0, .. }` -> assert the target is NOT `matches[0]`.
  - default fusion `SearchCodeOptions { w_bm25: 1.0, w_trigram: 1.0, w_cosine: 1.0, .. }` -> assert the target IS `matches[0]`.
  This deterministically proves fusion produced the rank, regardless of the model's absolute cosine ordering.
- Then do ONE `search code` MCP dispatch with the same query and assert the response wire shape: `matches[0]` is the target, `matches[0].score` present, `matches[0].signals.bm25 > 0.0` (or `trigram > 0.0`).

ALSO: update the existing `semantic_search_e2e.rs` assertions that read `m.get("similarity")` (the failure-message map around the `auth.rs` ranking assert) to read `score`/`signals` per the new response shape, so the existing e2e still compiles and passes.

### Gating
Gate this the same way the reference e2e is gated (real embedder under `#[serial_test::serial(cwd)]`). The always-run deterministic coverage for the fusion LOGIC lives in the search-crate / `search_code` unit tests (hand-written vectors); this card proves the end-to-end wiring with the real model.

## Acceptance Criteria
- [ ] New e2e runs through the real indexer + real embedder (no raw-SQL embedding inserts); asserts embeddings are present post-index.
- [ ] With cosine-only weights the target is NOT `matches[0]`; with default fusion weights the target IS `matches[0]` — same DB, same query embedding (the differential proof).
- [ ] A `search code` MCP dispatch returns the target at `matches[0]` with `score` present and `signals.bm25` (or `signals.trigram`) non-zero.
- [ ] The existing `semantic_search_e2e.rs` is migrated to the `score`/`signals` response shape and still passes.

## Tests
- [ ] `cargo test -p swissarmyhammer-tools --test integration search_fusion` (or the chosen test name) passes.
- [ ] `cargo test -p swissarmyhammer-tools --test integration semantic_search` still passes after the response-shape migration.

## Workflow
- Use `/tdd` — write the failing e2e first (cosine-only ranks the target below #1; fusion lifts it to #1), then rely on the implemented fusion to make it pass.