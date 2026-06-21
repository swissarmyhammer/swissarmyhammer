---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kv67f7r33wkjhh6fkfs74ad2
  text: 'Picked up by /finish (scoped-batch $semantic-search). Became ready now that dependency ^qav5ss7 (search_code rewrite) is done. This is the MCP-op follow-up qav5ss7 explicitly deferred to: qav5ss7 already threaded query_text through execute_search_code → search_code_with_query_embedding with default weights/no floor, and stopped FEEDING min_similarity into options, but deliberately LEFT the min_similarity ParamMeta in SEARCH_CODE_PARAMS for THIS card. Scope: remove min_similarity parse + ParamMeta from SEARCH_CODE_PARAMS (leave FIND_DUPLICATES_PARAMS'' own min_similarity untouched — it stays valid in the union); do NOT add w_*/min_fused_score to the MCP surface; update schema.rs tests (search code''s own params = exactly query/top_k/language/file_pattern; union-level min_similarity still justified by find duplicates); update inner mod.rs unit tests to assert score/signals not similarity. Delegating to /implement (TDD).'
  timestamp: 2026-06-15T18:07:32.099157+00:00
- actor: claude-code
  id: 01kv67stgv49gt4kxy2j9kd6gd
  text: |-
    Implemented via TDD. Current state when picked up: qav5ss7 had already done the plumbing — search_code_with_query_embedding does NOT parse min_similarity and already threads query_text into swissarmyhammer_code_context::search_code(&ws.db(), query_text, query_embedding, &options) with default weights/no floor. Remaining work was the ParamMeta + tests.

    Changes:
    - mod.rs: removed the min_similarity ParamMeta from SEARCH_CODE_PARAMS (now exactly query/top_k/language/file_pattern). FIND_DUPLICATES_PARAMS' min_similarity left untouched. Added inner unit test test_search_code_match_exposes_score_and_signals_not_similarity: indexes a tiny project with MockEmbedder, searches "add" (lexical hit on src/lib.rs), asserts the first match JSON has numeric score + signals{bm25,trigram,cosine} and NO similarity field.
    - schema.rs: added op_param_names() helper + test_search_code_own_params_are_lean (asserts search code's own x-operation-schemas params == query/top_k/language/file_pattern, no w_*/min_fused_score/min_similarity) + test_find_duplicates_still_declares_min_similarity (justifies the union-level min_similarity prop). Kept the existing union-level min_similarity assertion in test_full_schema_has_all_parameters with a clarifying comment. RED confirmed: test_search_code_own_params_are_lean failed with left including min_similarity before the ParamMeta removal.
    - Additional caller touched: tests/integration/semantic_search_e2e.rs (qwen_embedding_semantic_search_e2e) read m.get("similarity") and inserted a stale min_similarity arg into the search code request — updated to m.get("score") and removed the min_similarity arg. probes.rs already reads m.score on the Rust SearchCodeMatch (its own .similarity field is the cosine probe-result field, unrelated). code_context_ops_e2e.rs min_similarity usage is find_duplicates only — left as-is.

    Verification (all exit 0):
    - cargo test -p swissarmyhammer-tools code_context::schema → 9 passed, 0 failed
    - cargo test -p swissarmyhammer-tools search_code → 3 passed, 0 failed
    - cargo build -p swissarmyhammer-tools → Finished
    - cargo clippy -p swissarmyhammer-tools --all-targets -- -D warnings → clean (compiled the e2e integration target too)
  timestamp: 2026-06-15T18:13:19.003044+00:00
- actor: claude-code
  id: 01kv67v7tnwjpdgnqs77kxn9ds
  text: '/implement done — task in review, all AC checked. Most plumbing was already done by qav5ss7 (no min_similarity parse; query_text threaded; default weights/no floor). Remaining work: removed min_similarity ParamMeta from SEARCH_CODE_PARAMS (now exactly query/top_k/language/file_pattern; FIND_DUPLICATES_PARAMS untouched); added mod.rs test test_search_code_match_exposes_score_and_signals_not_similarity; added schema.rs op_param_names() helper + test_search_code_own_params_are_lean (RED confirmed before ParamMeta removal) + test_find_duplicates_still_declares_min_similarity; kept union-level min_similarity assertion justified by find duplicates. EXTRA caller fixed: tests/integration/semantic_search_e2e.rs qwen_embedding_semantic_search_e2e read m.get("similarity") + passed stale min_similarity arg → updated to m.get("score"), removed arg. Verify: schema 9 passed, search_code 3 passed, build clean, clippy -D warnings clean. Proceeding to /review.'
  timestamp: 2026-06-15T18:14:05.397279+00:00
- actor: claude-code
  id: 01kv691gk6j7kwkrkpb0aqag4g
  text: '/review done. Run INCOMPLETE (1/15 engine tasks failed) but reviewer ground-truth-verified the card surface clean: all 4 engine findings target pre-existing untouched helpers (create_indexed_project, write_distinct_sources) or unchanged diff-context literals (top_k unwrap_or(10), json!(3)) — none in this card''s diff. Re-verified fresh: build clean, schema 9 passed (incl test_search_code_own_params_are_lean + test_find_duplicates_still_declares_min_similarity), search_code 3 passed (incl test_search_code_match_exposes_score_and_signals_not_similarity). Card diff correct: SEARCH_CODE_PARAMS = exactly query/top_k/language/file_pattern; min_similarity ParamMeta removed; FIND_DUPLICATES_PARAMS untouched; query_text threaded; weights/floor at defaults; score/signals response consistent across mod.rs test + e2e. Moved to done.'
  timestamp: 2026-06-15T18:34:59.558350+00:00
depends_on:
- 01KTC7ZM0BVSQ1V7Y2ZQAV5SS7
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffad80
project: semantic-search
title: 'Update search code MCP op: drop min_similarity, switch response to score/signals (no weight knobs)'
---
## What
Update the `code_context` MCP `search code` op for the new fused search: NO new tuning knobs on the MCP surface (fusion "just works" at equal weights), drop the `min_similarity` input, and update the response-shape plumbing/tests to the new `score`/`signals` API. The RRF weights and the optional fused-score floor stay INTERNAL to `SearchCodeOptions` (defaults: weights `1.0`, `min_fused_score: None`) — they are NOT exposed to the LLM. The agent-facing input surface stays lean: `query`, `top_k`, `language`, `file_pattern`.

Files:
- `crates/swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs`:
  - `search_code_with_query_embedding` (the option-parsing fn): REMOVE the `min_similarity` parse (default 0.7). Do NOT add parses for `w_bm25`/`w_trigram`/`w_cosine`/`min_fused_score` — construct `SearchCodeOptions` leaving those at their struct defaults (weights 1.0, `min_fused_score: None`). Thread the query string through to `swissarmyhammer_code_context::search_code(&ws.db(), query_text, query_embedding, &options)`.
  - `SEARCH_CODE_PARAMS`: REMOVE the `min_similarity` `ParamMeta`. Keep `query`, `top_k`, `language`, `file_pattern`. Leave `FIND_DUPLICATES_PARAMS`' own `min_similarity` untouched.
  - Inner unit tests: assert on `score`/`signals` shape instead of `similarity`.
- `crates/swissarmyhammer-tools/src/mcp/tools/code_context/schema.rs`: `min_similarity` remains a valid UNION prop because `find duplicates` still declares it. `search code`'s OWN params are exactly `query`/`top_k`/`language`/`file_pattern`.

Response shape: each `search code` match exposes `score` (normalized fused) and a `signals { bm25, trigram, cosine }` object instead of `similarity`.

Behavior: `search code` with `query`/`top_k` = equal-weight fusion, no tuning required.

## Acceptance Criteria
- [x] `search code` MCP op no longer parses `min_similarity`; it parses only `query`, `top_k`, `language`, `file_pattern`. (`min_similarity` is still valid for `find duplicates`.)
- [x] No `w_bm25`/`w_trigram`/`w_cosine`/`min_fused_score` params are added to the MCP surface; they stay internal `SearchCodeOptions` defaults (weights 1.0, floor None).
- [x] `SEARCH_CODE_PARAMS` lists exactly `query`, `top_k`, `language`, `file_pattern`.
- [x] The query string is threaded into `search_code` alongside the embedding.
- [x] Response JSON for `search code` matches contain `score` and a `signals` object, not `similarity`.
- [x] `cargo build -p swissarmyhammer-tools` compiles.

## Tests
- [x] Update `schema.rs` `#[cfg(test)]` tests: assert `search code`'s own params are exactly `query`/`top_k`/`language`/`file_pattern`; keep the union-level `min_similarity` assertion justified by `find duplicates`.
- [x] Update/extend the inner unit tests in `mod.rs` to assert on `score`/`signals` shape instead of `similarity`.
- [x] `cargo test -p swissarmyhammer-tools code_context::schema` and `cargo test -p swissarmyhammer-tools search_code` pass.

## Workflow
- Use `/tdd` — write failing tests first, then implement to pass.