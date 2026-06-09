---
assignees:
- claude-code
depends_on:
- 01KTC7ZM0BVSQ1V7Y2ZQAV5SS7
position_column: todo
position_ordinal: 8c80
project: semantic-search
title: 'Update search code MCP op: drop min_similarity, switch response to score/signals (no weight knobs)'
---
## What
Update the `code_context` MCP `search code` op for the new fused search: NO new tuning knobs on the MCP surface (fusion "just works" at equal weights), drop the `min_similarity` input, and update the response-shape plumbing/tests to the new `score`/`signals` API. The RRF weights and the optional fused-score floor stay INTERNAL to `SearchCodeOptions` (defaults: weights `1.0`, `min_fused_score: None`) — they are NOT exposed to the LLM. The agent-facing input surface stays lean: `query`, `top_k`, `language`, `file_pattern`.

Files:
- `crates/swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs`:
  - `search_code_with_query_embedding` (the option-parsing fn, ~line 1525): REMOVE the `min_similarity` parse (default 0.7). Do NOT add parses for `w_bm25`/`w_trigram`/`w_cosine`/`min_fused_score` — construct `SearchCodeOptions` leaving those at their struct defaults (weights 1.0, `min_fused_score: None`). This fn is called from `search_code` (~line 1511) which already has the embedded query text in scope — thread that query string through to `swissarmyhammer_code_context::search_code(&ws.db(), query_text, query_embedding, &options)` per the search_code-rewrite card's new signature.
  - `SEARCH_CODE_PARAMS` (the `&[ParamMeta]` static near `pub struct SearchCode;`, ~line 432): REMOVE the `min_similarity` `ParamMeta`. Do NOT add weight/floor params. Keep `query`, `top_k`, `language`, `file_pattern`. Leave `FIND_DUPLICATES_PARAMS`' own `min_similarity` untouched.
  - The inner unit test `test_search_code_returns_result_with_progress_when_not_embedded` (and the direct `search_code_with_query_embedding` test around line 4466) read through `serde_json::Value` — update any `similarity` reads to `score`/`signals`.
- `crates/swissarmyhammer-tools/src/mcp/tools/code_context/schema.rs`: the `search code` example `{"op":"search code","query":...,"top_k":5}` is still correct as-is. IMPORTANT: do NOT delete the `min_similarity` schema assertion blindly — `min_similarity` remains a valid prop in the UNION because `find duplicates` (`FIND_DUPLICATES_PARAMS`) still declares it. Update assertions to reflect that `search code`'s OWN params are exactly `query`/`top_k`/`language`/`file_pattern` (no `w_*`, no `min_fused_score`, no `min_similarity`).

Response shape (kept rich — cheap relative to chunk text, and the e2e cards assert on it): each `search code` match exposes `score` (normalized fused) and a `signals { bm25, trigram, cosine }` object instead of `similarity`.

Behavior: `search code` with `query`/`top_k` = equal-weight fusion, no tuning required.

## Acceptance Criteria
- [ ] `search code` MCP op no longer parses `min_similarity`; it parses only `query`, `top_k`, `language`, `file_pattern`. (`min_similarity` is still valid for `find duplicates`.)
- [ ] No `w_bm25`/`w_trigram`/`w_cosine`/`min_fused_score` params are added to the MCP surface; they stay internal `SearchCodeOptions` defaults (weights 1.0, floor None).
- [ ] `SEARCH_CODE_PARAMS` lists exactly `query`, `top_k`, `language`, `file_pattern`.
- [ ] The query string is threaded into `search_code` alongside the embedding.
- [ ] Response JSON for `search code` matches contain `score` and a `signals` object, not `similarity`.
- [ ] `cargo build -p swissarmyhammer-tools` compiles.

## Tests
- [ ] Update `schema.rs` `#[cfg(test)]` tests: assert `search code`'s own params are exactly `query`/`top_k`/`language`/`file_pattern`; keep the union-level `min_similarity` assertion justified by `find duplicates`.
- [ ] Update/extend the inner unit tests in `mod.rs` to assert on `score`/`signals` shape instead of `similarity`.
- [ ] `cargo test -p swissarmyhammer-tools code_context::schema` and `cargo test -p swissarmyhammer-tools search_code` pass.

## Workflow
- Use `/tdd` — write failing tests first, then implement to pass.