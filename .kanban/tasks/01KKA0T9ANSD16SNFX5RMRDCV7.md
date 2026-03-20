---
position_column: done
position_ordinal: b380
title: Add find duplicates operation
---
## What

The spec defines `find duplicates` — detect clusters of semantically similar code chunks. The library function `find_duplicates_in_file()` exists in `swissarmyhammer-treesitter/src/unified.rs` but is not exposed as an MCP tool operation.

**Key files:**
- `swissarmyhammer-treesitter/src/unified.rs` — `find_duplicates_in_file()` (exists, tested)
- `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs` — add operation
- `swissarmyhammer-code-context/src/ops/` — add find_duplicates.rs

**Approach:**
1. Add `find duplicates` operation to code_context tool
2. Use embedding cosine similarity across chunks
3. Parameters: `min_similarity` (default 0.85), `min_chunk_bytes` (default 100), `file` (optional)
4. Group results into clusters of similar chunks

## Acceptance Criteria
- [ ] `find duplicates` operation registered and callable
- [ ] Returns clusters of similar chunks with similarity scores
- [ ] Respects min_similarity threshold and min_chunk_bytes filter
- [ ] Can scope to a single file or search entire workspace

## Tests
- [ ] Unit test with known duplicate code
- [ ] `cargo test -p swissarmyhammer-tools` passes