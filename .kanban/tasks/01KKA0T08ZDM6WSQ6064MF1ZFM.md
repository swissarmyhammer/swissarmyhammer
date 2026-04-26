---
position_column: done
position_ordinal: b580
title: Add search code operation (embedding similarity)
---
## What

The spec defines `search code` — semantic similarity search using embeddings + cosine similarity. The embedding infrastructure exists (`ane-embedding`, `llama-embedding`, `model-embedding` crates) and embeddings are computed during `scan()`, but there's no MCP tool operation to query by similarity.

**Key files:**
- `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs` — add new operation
- `swissarmyhammer-code-context/src/ops/` — add search_code.rs
- `swissarmyhammer-treesitter/src/index.rs` — embeddings stored in `ts_chunks.embedding`

**Approach:**
1. Add `search code` operation to the code_context tool
2. Embed the query text using the same model
3. Compute cosine similarity against all chunk embeddings
4. Return top-k results above min_similarity threshold
5. Parameters: `query`, `top_k` (default 10), `min_similarity` (default 0.7)

## Acceptance Criteria
- [ ] `search code` operation registered and callable
- [ ] Returns semantically similar chunks ranked by cosine similarity
- [ ] Chunks include file path, line range, symbol_path, similarity score
- [ ] Respects `top_k` and `min_similarity` parameters

## Tests
- [ ] Unit test: cosine similarity ranking with known embeddings
- [ ] `cargo test -p swissarmyhammer-code-context` passes
- [ ] `cargo test -p swissarmyhammer-tools` passes