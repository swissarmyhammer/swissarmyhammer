---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kv692crfaykbss2e7v7awwgq
  text: 'Picked up by /finish (scoped-batch $semantic-search). Became ready now that dependency ^z9r1bcq is done. Keystone REAL-pipeline e2e card (tag #1): real index_discovered_files_async + real embedder (GPU available), NO fixture/raw-SQL embedding inserts. Proof strategy = WEIGHT DIFFERENTIAL on the SAME indexed DB (not engineering model ranking): typo query (e.g. reticulate_splne) embedded once, search_code called twice — cosine-only {w_bm25:0,w_trigram:0,w_cosine:1} → target NOT matches[0]; default fusion {1,1,1} → target IS matches[0]. Then one MCP `search code` dispatch asserts wire shape: matches[0]=target, score present, signals.bm25 (or trigram) > 0. ALSO migrate existing semantic_search_e2e.rs m.get("similarity") reads → score/signals (note: z9r1bcq''s implementer already migrated the qwen_embedding_semantic_search_e2e diagnostic; check whether the auth.rs ranking-assert failure map still reads "similarity"). Gate under #[serial_test::serial(cwd)] like the reference e2e. Delegating to /implement (TDD).'
  timestamp: 2026-06-15T18:35:28.399063+00:00
- actor: claude-code
  id: 01kv69hxw4esta40zwz9v5e0s9
  text: |-
    Implemented the keystone fusion e2e via TDD. New file: crates/swissarmyhammer-tools/tests/integration/search_fusion_e2e.rs, test `qwen_embedding_search_fusion_rescues_typo_e2e` (registered in tests/integration/mod.rs). Real index_discovered_files_async + real Embedder::default(); asserts embeddings present (ts_chunks.embedding NOT NULL, indexed_files.embedded=1 for all 5 files). NO raw-SQL inserts.

    Differential proof on the SAME DB + SAME query embedding:
    - Corpus: target src/render.rs with distinctive `fn reticulate_splines` and a deliberately TERSE/generic body (weak embedding); a SEMANTIC decoy src/geometry.rs `fn tessellate_curve_patches` whose DOC COMMENT richly describes spline/mesh rendering; plus ordinary decoys (auth/parser/math).
    - Query = "subdivide the surface mesh and rasterize the geometry for the render pass reticulate_splne". The prose is drawn from the decoy's doc comment (pulls the EMBEDDING toward geometry.rs; those words are NOT in any symbol_path so they give the decoy no lexical edge), and `reticulate_splne` is a typo of the target identifier (shares the long prefix `reticulate_spl`, giving near-maximal char-trigram Dice on the target's high-weight symbol_path).
    - Measured on the real qwen model: cosine-only ranks geometry.rs (~0.635) ABOVE the target render.rs (~0.567) -> target NOT matches[0]. Default fusion {1,1,1} ranks render.rs #1 (bm25 ~4.66, trigram ~1.97 on its symbol_path) -> target IS matches[0]. This is why the typo defeats cosine but trigram rescues it.
    - One `search code` MCP dispatch with the same query: matches[0] is the target, score present, signals.bm25 (or trigram) > 0.

    Why earlier corpus designs failed (recorded so the next agent doesn't repeat): a bare typo query `reticulate_splne` still embeds CLOSEST to the target chunk because the embedder tokenizes the identifier subwords and the typo preserves them (cosine-only put render.rs #1). Adding the decoy's IDENTIFIER words to the query fixed cosine but then the decoy also won the lexical signal (its symbol_path matched), so fusion still ranked the decoy #1. The working design pulls cosine using only decoy PROSE words (absent from symbol_paths), keeping the lexical field clean for the target.

    NOTE: the "migrate similarity->score/signals" part was already complete — semantic_search_e2e.rs's auth.rs ranking failure-map already reads `score` (done by dependency z9r1bcq). No remaining m.get("similarity") in integration tests (only find_duplicates' legitimate min_similarity).

    Verification (all green):
    - cargo test -p swissarmyhammer-tools --test tools_tests search_fusion -> 1 passed
    - cargo test -p swissarmyhammer-tools --test tools_tests semantic_search -> 1 passed
    - cargo build -p swissarmyhammer-tools -> ok
    - cargo clippy -p swissarmyhammer-tools --all-targets -- -D warnings -> clean
  timestamp: 2026-06-15T18:43:57.444833+00:00
- actor: claude-code
  id: 01kv69kh43z6z85e9r1qeaf6gw
  text: '/implement done — task in review, all AC checked. New test crates/swissarmyhammer-tools/tests/integration/search_fusion_e2e.rs::qwen_embedding_search_fusion_rescues_typo_e2e (gated #[serial_test::serial(cwd)], qwen_embedding_ prefix so default nextest filter excludes it; registered in integration/mod.rs). REAL index_discovered_files_async + real Embedder::default() (no raw-SQL embeddings); asserts ts_chunks.embedding NOT NULL + indexed_files.embedded=1 for all 5 files. Differential proof on same DB/same query embedding: query = decoy prose (pulls embedding toward geometry.rs semantic decoy) + typo reticulate_splne (shares prefix reticulate_spl with target symbol_path → high trigram Dice but not a real token cosine can recover). Measured: cosine-only ranks geometry.rs ~0.635 > target render.rs ~0.567 → target NOT matches[0]; fusion {1,1,1} → target IS matches[0] (bm25 ~4.66, trigram ~1.97). MCP dispatch: matches[0]=target, score present, signals.bm25>0. Two failed corpus iterations documented (bare typo still embeds closest to target). similarity→score migration already done by z9r1bcq; verified no m.get("similarity") reads remain. Verify: search_fusion 1 passed, semantic_search 1 passed, build + clippy -D warnings clean. Proceeding to /review.'
  timestamp: 2026-06-15T18:44:49.923974+00:00
- actor: claude-code
  id: 01kv6a7keskqnc5x9mw4bzr4t1
  text: '/review done. Run COMPLETE (15/15, 0 failed): 0 blockers, 5 warnings, 2 nits — none genuine in-scope. Warnings = pre-existing helper copy-paste (make_context_with_dir/count_embedded_chunks/read_index_flags/extract_text) that mirrors the reference semantic_search_e2e.rs per-file private-helper convention (the card mandated mirroring it); extracting a shared module is a suite-wide refactor across 3 files, out of scope. Nits refuted: hardcoded 4 is inside a raw-string decoy source literal (geometry.rs corpus content, not control flow); write_corpus length driven by 5 inline raw-string corpora co-located with load-bearing design rationale. Reviewer confirmed the keystone test is GENUINE + load-bearing: real index_discovered_files_async + Embedder::default(), no raw-SQL inserts; embeddings asserted present (count>0, (ts_indexed,embedded)=(1,1) ×5); differential sound (same shared_db + same query_embedding, only weights vary; assert_ne cosine_rank≠0 vs assert_eq target_rank=0); MCP wire-shape meaningful; gating correct. Moved to done.'
  timestamp: 2026-06-15T18:55:47.673419+00:00
depends_on:
- 01KTC808B8H9PX5DXPXZ9R1BCQ
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffae80
project: semantic-search
title: 'Real-pipeline e2e: exact-identifier query that cosine-only misses now ranks top via fusion'
---
## What
Add the keystone end-to-end test proving the fused `search code` rescues a query that pure-cosine misses. Runs through the REAL indexer and REAL embedder following the reference pattern in `crates/swissarmyhammer-tools/tests/integration/semantic_search_e2e.rs` (real `index_discovered_files_async` -> real `search_code` / registered MCP dispatch). NOT fixture-only (no raw-SQL-inserted embeddings). The embedder is always available (the Test runner has a GPU) — embed for real; there is no model-free path.

### Proof strategy: a WEIGHT DIFFERENTIAL on an ordinary corpus
Do NOT try to engineer files so the embedding model ranks them a certain way — you cannot reliably predict the model's absolute cosine ranking. Instead prove fusion changed the outcome by flipping the internal weights on the SAME indexed DB. `SearchCodeOptions` weights are public fields the integration test can set directly. This works on any small ordinary corpus; no special "adversarial" files are needed.

## Acceptance Criteria
- [x] New e2e runs through the real indexer + real embedder (no raw-SQL embedding inserts); asserts embeddings are present post-index.
- [x] With cosine-only weights the target is NOT `matches[0]`; with default fusion weights the target IS `matches[0]` — same DB, same query embedding (the differential proof).
- [x] A `search code` MCP dispatch returns the target at `matches[0]` with `score` present and `signals.bm25` (or `signals.trigram`) non-zero.
- [x] The existing `semantic_search_e2e.rs` is migrated to the `score`/`signals` response shape and still passes. (Was already migrated by dependency z9r1bcq; verified no `m.get("similarity")` reads remain in integration tests.)

## Tests
- [x] `cargo test -p swissarmyhammer-tools --test tools_tests search_fusion` passes (test `qwen_embedding_search_fusion_rescues_typo_e2e`).
- [x] `cargo test -p swissarmyhammer-tools --test tools_tests semantic_search` still passes after the response-shape migration.

## Implementation
New file `crates/swissarmyhammer-tools/tests/integration/search_fusion_e2e.rs`. Corpus: target `src/render.rs` (`fn reticulate_splines`, terse body = weak embedding) + semantic decoy `src/geometry.rs` (`fn tessellate_curve_patches`, rich spline/mesh-rendering doc comment) + ordinary decoys. Query mixes decoy PROSE words (pull cosine toward geometry.rs, absent from any symbol_path) with the typo `reticulate_splne` (near-maximal char-trigram on the target's high-weight symbol_path). Measured: cosine-only ranks geometry.rs above the target; default fusion ranks the target #1.

## Workflow
- Used `/tdd` — wrote the failing e2e first, iterated the corpus/query so cosine genuinely misses, then default fusion passes.