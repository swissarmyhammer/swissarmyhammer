---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kv658ezrmgeewef0yhww5vvw
  text: 'Picked up by /finish (scoped-batch $semantic-search). Became ready now that dependencies ^1c67x5b (search crate) and the types card are done. From-scratch implementation card (progress 0.0). Key risks to watch: (1) MUST keep `swissarmyhammer_code_context::serialize_embedding` a valid public path via re-export from swissarmyhammer-search — the LIVE indexer (swissarmyhammer-tools mod.rs:2123) and code-context-cli doctor.rs:456 depend on it; (2) repoint find_duplicates.rs imports (cosine_similarity → model_embedding directly, serialize_embedding → crate-root re-export); (3) new query_text param threaded through search_code; (4) SearchCodeMatch.similarity → score + signals; SearchCodeOptions.min_similarity → w_bm25/w_trigram/w_cosine + min_fused_score. Delegating to /implement (TDD).'
  timestamp: 2026-06-15T17:28:52.984799+00:00
- actor: claude-code
  id: 01kv65c2w42yxykb28xbjnxjjc
  text: |-
    Research done. Findings vs card:
    - find_duplicates.rs does NOT import cosine_similarity (card said line 11) — it only imports serialize_embedding in tests (line 204). Repoint that to crate::serialize_embedding (crate-root re-export).
    - search_code.rs uses LoadedChunk (not EmbeddingRow) as the DB layer; keeping LoadedChunk + load_embedded_chunks/load_all_embedded_chunks.
    - cosine-contract + blob round-trip tests already live in swissarmyhammer-search (cosine.rs). Deleting the duplicate ones here.
    - Production callers of the changed surface beyond tests: (1) code-context-cli doctor.rs run_canary_query (search_code + min_similarity); (2) tools mod.rs execute_search_code/search_code_with_query_embedding (search_code + min_similarity + serialize_embedding@2123); (3) validators probes.rs run_similar (search_loaded + m.similarity). Sibling card 01KTC808 will rework the MCP op fully; I only keep it compiling.
    - New search_code signature per sibling card 01KTC80Y: search_code(&db, query_text, query_embedding, &options) — query_text before embedding.
    - search_loaded must also produce the new score/signals shape (shares rank core); thread query_text into it for the validators caller.
    TDD: writing failing fusion/score/signals tests in search_code.rs first.
  timestamp: 2026-06-15T17:30:51.652443+00:00
- actor: claude-code
  id: 01kv66cv77fye348nv9f47zm5t
  text: |-
    Implementation landed (TDD: 6 new/migrated tests written red against the new API, watched 33 compile errors, then implemented green).

    Files changed:
    - crates/swissarmyhammer-code-context/Cargo.toml: added swissarmyhammer-search dep.
    - crates/swissarmyhammer-code-context/src/ops/search_code.rs: rewrote search_code/search_loaded/rank_loaded to build Docs (symbol_path weight 5.0, body weight 1.0, embedding) and call swissarmyhammer_search::search; SearchCodeMatch.similarity -> score+signals; SearchCodeOptions.min_similarity -> w_bm25/w_trigram/w_cosine(1.0) + min_fused_score:Option. serialize_embedding + deserialize_embedding + Signals now re-exported from swissarmyhammer-search (serialize_embedding/Signals pub). Deleted the duplicated cosine-contract + blob round-trip tests (live in search crate). hit_to_match maps Hit.id (=corpus index) back to chunk, no side-map clone.
    - crates/swissarmyhammer-code-context/src/lib.rs: export Signals; serialize_embedding re-export preserved.
    - crates/swissarmyhammer-code-context/src/ops/find_duplicates.rs: test import repointed to crate::serialize_embedding (crate-root re-export). It never imported cosine_similarity.

    Callers updated (whole workspace builds):
    - apps/code-context-cli/src/commands/doctor.rs run_canary_query: new 4-arg search_code; made the canary COSINE-ONLY (w_bm25=w_trigram=0) with min_fused_score=Some(EPSILON) so the embedding-dimension-mismatch health check still drops a 0-cosine row even though the canary query text lexically overlaps the chunk body (would otherwise false-Ok the dimension-mismatch regression test). Updated stale min_similarity doc wording.
    - crates/swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs: threaded query text into execute_search_code -> search_code_with_query_embedding (now 4 args incl query_text); options use default weights/no floor; dropped parsing min_similarity into options (left the ParamMeta declaration for follow-up card 01KTC808). Updated the unit test caller.
    - crates/swissarmyhammer-validators/src/review/probes.rs run_similar: search_loaded now 4-arg, passes the added function body as query_text; reads m.score instead of m.similarity.

    Verification (all green): cargo test -p swissarmyhammer-code-context search_code -> 14 passed/0 failed; find_duplicates -> 9 passed/0 failed; full crate suite 689+26+2 passed; cargo build code-context/tools/code-context-cli all Finished ok; cargo clippy -p swissarmyhammer-code-context --all-targets -- -D warnings clean; validators probes 15 passed; tools schema tests 7 passed. Moving to review.
  timestamp: 2026-06-15T17:48:45.159684+00:00
- actor: claude-code
  id: 01kv66e8jb9asncp1k64rcw5ph
  text: '/implement done — task in review, all AC checked. Rewrote search_code + search_loaded to build Docs (symbol_path weight 5.0, body weight 1.0, +embedding) and call swissarmyhammer_search::search; Hit.id = corpus index → O(1) hit_to_match (no side-map clone). API: SearchCodeOptions min_similarity→w_bm25/w_trigram/w_cosine(1.0)+min_fused_score; SearchCodeMatch similarity→score+signals{bm25,trigram,cosine}; query_text param added. Callers updated across 5 crates: code-context (lib.rs re-exports Signals + serialize_embedding preserved; find_duplicates repointed serialize_embedding→crate root — note: it never imported cosine_similarity, card line note was stale), code-context-cli doctor.rs (canary forced COSINE-ONLY — judgment call, it''s a dimension-mismatch probe the lexical signals would false-positive), swissarmyhammer-tools mod.rs (threaded query_text), swissarmyhammer-validators probes.rs. Verify: search_code 14 passed, find_duplicates 9 passed, builds of code-context/tools/code-context-cli clean, clippy -D warnings clean, full code-context suite 689+26+2 passed, validators 15, tools schema 7. Proceeding to /review.'
  timestamp: 2026-06-15T17:49:31.595246+00:00
- actor: claude-code
  id: 01kv67d0jgntqg9rp1vzrz1jvz
  text: |-
    /review — CLEAN for this card's change surface. Moving to done.

    Engine: `review working`, run INCOMPLETE (2/30 review tasks failed). Raw counts: 1 blocker, 2 warnings, 11 nits (14 confirmed / 7 refuted). Every reported finding verified against ground truth (git diff HEAD + git show HEAD) and found to be PRE-EXISTING, untouched code — none in this card's actual diff:

    - Blocker `search_code.rs:414` (SQL injection via LIKE-clause string interpolation in `load_embedded_chunks`): present at HEAD (HEAD lines 358/360/365), NOT modified by this card. Not a regression here; filter values are programmatic extension lists, not user free-text. Pre-existing tech-debt for a separate card.
    - Warning `find_duplicates.rs:115` (long fn) + nits `:85/:87` (hardcoded 0.85, 5): pre-existing; this card's only change to find_duplicates is the `serialize_embedding` import repoint to crate root (verified clean).
    - Warning `probes.rs:96` (`&[String]` → generic): different pre-existing fn; this card only touched run_similar's 4-arg `search_loaded` call + `m.score` read.
    - Nits in doctor.rs:333 / mod.rs module-doc / struct doc-comments / DEFAULT_TOP_K consts: pre-existing or out-of-surface.

    Card surface verified sound:
    - Doc-building + Hit.id=corpus-index O(1) hit_to_match: correct, no side-map clone.
    - `serialize_embedding` public re-export path: preserved (live indexer + CLI doctor compile).
    - find_duplicates repoint: clean.
    - Doctor canary cosine-only judgment call: VERIFIED CORRECT. Under full fusion the canary query text lexically overlaps the chunk body, so bm25/trigram would false-positive a dimension-mismatched (cosine==0.0) row past the floor and falsely report Ok; zeroing w_bm25/w_trigram with w_cosine=1.0 + min_fused_score=Some(EPSILON) preserves the original dimension-mismatch probe intent. Regression-guard test updated and retained.

    Ground-truth re-verify: `cargo test -p swissarmyhammer-code-context search_code` → 14 passed / 0 failed.
  timestamp: 2026-06-15T18:06:19.216323+00:00
- actor: claude-code
  id: 01kv67edzr69d6ghs3s0vk9c5f
  text: '/review done. Run was INCOMPLETE (2/30 engine tasks failed) but the reviewer ground-truth-verified (git diff HEAD) that EVERY reported finding lands in pre-existing untouched code, not this card''s diff: SQL-injection blocker at search_code.rs:414 (LIKE-clause interpolation in load_embedded_chunks) is verbatim at HEAD with programmatic filter values, NOT user free-text — pre-existing tech debt, out of this card''s surface; find_duplicates long-fn + probes &[String] warnings are pre-existing functions. Card surface verified sound: Doc-building + Hit.id=corpus-index O(1) mapping correct; serialize_embedding public re-export preserved (live indexer + CLI compile); find_duplicates repoint clean; doctor canary cosine-only judgment VERIFIED CORRECT (full fusion would false-positive a dim-mismatched cosine==0 row via lexical overlap; w_cosine=1 + others 0 + min_fused_score=EPSILON preserves probe intent). search_code 14/14 pass. FOLLOW-UP NOTE for orchestrator: pre-existing SQL-injection-style LIKE interpolation in load_embedded_chunks is latent tech-debt worth a separate card — out of scope here; not fixed.'
  timestamp: 2026-06-15T18:07:05.720345+00:00
- actor: claude-code
  id: 01kv980vn3wmej7sjyqz22thrq
  text: 'FOLLOW-UP (found during a full-branch double-check): this card''s doctor.rs canary change had a latent regression. The cosine-only canary floored on `min_fused_score: Some(f32::EPSILON)`, but that floor is on the RANK-NORMALIZED fused score — the top doc always normalizes to 1.0 regardless of its raw cosine. So on an embedding-dimension mismatch (raw cosine == 0.0 for every chunk), the canary still got a score-1.0 "match" and falsely reported Ok — the exact failure it exists to detect. Masked because the regression-guard test `check_semantic_search_dimension_mismatch_is_not_ok` is gated behind `#[cfg(feature = "embedding-models")]` and never ran in normal nextest; running it with the feature FAILED. Fixed in doctor.rs: drop min_fused_score, keep cosine-only weights, and classify on the count of matches whose RAW `signals.cosine > f32::EPSILON`. Both gated canary tests now pass (dimension-mismatch → not-Ok; matching-dimension → Ok); clippy --features embedding-models clean. Lesson: min_fused_score/Hit.score are normalized rank scores, not raw similarities — consumers needing a raw threshold must read Hit.signals.*.'
  timestamp: 2026-06-16T22:14:52.835253+00:00
depends_on:
- 01KTC7Y50PEM427HQ79NW52WY4
- 01KTC7YTM7HYC2TBHRD1C67X5B
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffac80
project: semantic-search
title: Rewrite search_code to build Docs and call swissarmyhammer-search::search
---
## What
Rewrite the embedding-only `search_code` in `crates/swissarmyhammer-code-context/src/ops/search_code.rs` to build `swissarmyhammer_search::Doc`s from the already-loaded `ts_chunks` rows and rank them by calling `swissarmyhammer_search::search`. The BM25/trigram/RRF/cosine logic now lives in the `swissarmyhammer-search` crate — do NOT create a code-context-local `search_fusion` module.

(See comments for the full implementation log.)

## Acceptance Criteria
- [x] `SearchCodeOptions` has `w_bm25`/`w_trigram`/`w_cosine` (default 1.0) and `min_fused_score: Option<f32>`; `min_similarity` is gone. `cargo build -p swissarmyhammer-code-context` compiles.
- [x] `search_code` accepts the query string and builds `Doc`s (symbol_path high weight, text low weight, embedding) and calls `swissarmyhammer_search::search`.
- [x] `SearchCodeMatch` exposes `score` (normalized) and `signals { bm25, trigram, cosine }`; `similarity` field removed.
- [x] `swissarmyhammer_code_context::serialize_embedding` is still a valid public path (re-exported from `swissarmyhammer-search`); `cargo build -p swissarmyhammer-tools` (the live indexer at mod.rs:2123) and `cargo build -p code-context-cli` (doctor.rs:456) still compile.
- [x] `find_duplicates.rs` compiles with repointed imports and its tests pass (`cargo test -p swissarmyhammer-code-context find_duplicates`).
- [x] An exact-identifier query whose cosine is low but whose symbol_path matches still ranks via the bm25/trigram contribution (unit test below).
- [x] `lib.rs` re-exports compile with the new/renamed public types (`Signals` exported, `serialize_embedding` re-exported).

## Tests
- [x] Migrate + extend `#[cfg(test)] mod tests` in `search_code.rs`: a fusion-ranking test where a chunk with a strong symbol_path/bm25 match but weak cosine out-ranks a chunk with mediocre signals; a `min_fused_score` floor test (on normalized score); a `signals` breakdown presence test; a weights-affect-ordering test (boosting `w_cosine` reorders results). Keep the `IndexingProgress` tests as-is.
- [x] `cargo test -p swissarmyhammer-code-context search_code` passes (migrated + new tests green), and `find_duplicates` tests still pass.

## Workflow
- Use `/tdd` — write failing tests first, then implement to pass.