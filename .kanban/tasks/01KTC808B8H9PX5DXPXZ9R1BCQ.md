---
assignees:
- claude-code
depends_on:
- 01KTC7ZM0BVSQ1V7Y2ZQAV5SS7
position_column: todo
position_ordinal: 8c80
project: semantic-search
title: Thread fusion weights and fused-score floor through the search code MCP wrapper and schema
---
## What
Expose the new fused-search options through the `code_context` MCP `search code` op and update the response-shape plumbing/tests to the new `score`/`signals` API. Files:
- `crates/swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs`:
  - `search_code_with_query_embedding` (the option-parsing fn, ~line 1525): REMOVE the `min_similarity` parse (default 0.7). ADD parses for `w_bm25`, `w_trigram`, `w_cosine` (f64 -> f32, each default `1.0`) and `min_fused_score` (optional f32, `None` when absent). Build the new `SearchCodeOptions` (no `min_similarity`). This fn is called from `search_code` (~line 1511) which already has the embedded query text in scope — thread that query string through to `swissarmyhammer_code_context::search_code(&ws.db(), query_text, query_embedding, &options)` per the search_code-rewrite card's new signature.
  - `SEARCH_CODE_PARAMS` (the `&[ParamMeta]` static near `pub struct SearchCode;`, ~line 432): REMOVE the `min_similarity` `ParamMeta`. ADD `ParamMeta`s for `w_bm25`, `w_trigram`, `w_cosine` (ParamType::Number, "default 1.0", describe as RRF signal weights) and `min_fused_score` (ParamType::Number, optional fused-score floor). Keep `query`, `top_k`, `language`, `file_pattern`. Leave `FIND_DUPLICATES_PARAMS`' own `min_similarity` untouched.
  - The inner unit test `test_search_code_returns_result_with_progress_when_not_embedded` (and the direct `search_code_with_query_embedding` test around line 4466) read through `serde_json::Value` — update any `similarity` reads to `score`/`signals`, and exercise passing a `w_cosine` weight.
- `crates/swissarmyhammer-tools/src/mcp/tools/code_context/schema.rs`: the `search code` example currently shows `{"op":"search code","query":...,"top_k":5}` — leave query/top_k, optionally add a `w_cosine`/`min_fused_score` example. IMPORTANT: do NOT delete the `min_similarity` schema assertion blindly — `min_similarity` remains a valid prop in the union because `find duplicates` (`FIND_DUPLICATES_PARAMS`) still declares it. Add assertions that the union contains `w_bm25`, `w_trigram`, `w_cosine`, `min_fused_score`.

Behavior: `search code` with no weight args = equal-weight fusion; passing `w_cosine` etc. tunes ranking; `min_fused_score` applies the optional floor.

## Acceptance Criteria
- [ ] `search code` MCP op accepts `w_bm25`, `w_trigram`, `w_cosine`, `min_fused_score`; `min_similarity` is no longer parsed for `search code` (still valid for `find duplicates`).
- [ ] `SEARCH_CODE_PARAMS` lists the four new params and not `min_similarity`.
- [ ] The query string is threaded into `search_code` alongside the embedding.
- [ ] Generated schema's property union contains `w_bm25`/`w_trigram`/`w_cosine`/`min_fused_score`.
- [ ] Response JSON for `search code` matches contain `score` and a `signals` object, not `similarity`.
- [ ] `cargo build -p swissarmyhammer-tools` compiles.

## Tests
- [ ] Update `crates/swissarmyhammer-tools/src/mcp/tools/code_context/schema.rs` `#[cfg(test)]` tests: assert the new params appear in the property union; keep the `min_similarity` assertion justified by `find duplicates`.
- [ ] Update/extend the inner unit tests in `mod.rs` to assert on `score`/`signals` shape instead of `similarity`, and to exercise passing a `w_cosine` weight.
- [ ] `cargo test -p swissarmyhammer-tools code_context::schema` and `cargo test -p swissarmyhammer-tools search_code` pass.

## Workflow
- Use `/tdd` — write failing tests first, then implement to pass.