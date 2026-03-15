---
position_column: done
position_ordinal: a1
title: Incremental DB writes during tree-sitter scan
---
## What

`index_discovered_files_async` currently calls `ts_index.scan().await` which parses+embeds ALL 1216 files before returning. Only then does it iterate and write chunks to the code-context DB. This means status shows 0 progress for minutes.

Change the architecture so chunks, symbols, and edges are written to the DB incrementally as each file finishes parsing, not in a batch at the end.

**Key files:**
- `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs` — `index_discovered_files_async` (line 630+)
- `swissarmyhammer-treesitter/src/index.rs` — `IndexContext::scan()` and `IndexContext::index_file()`

**Approach:** Instead of calling `scan()` (which does everything), iterate over discovered files and call per-file parse+chunk. After each file, immediately write chunks/symbols/edges to the DB and mark `ts_indexed=1`. This way status shows real-time progress.

The embedding step can remain lazy (embeddings written later) since the chunks and symbols are the critical data for queries.

## Acceptance Criteria
- [ ] `get status` shows incremental progress while indexing is running
- [ ] Chunks appear in DB within seconds of MCP startup, not minutes
- [ ] Files are marked `ts_indexed=1` individually as they complete
- [ ] No regression in chunk quality (symbol_path populated where applicable)

## Tests
- [ ] Unit test: mock DB + single file parse → chunks written immediately
- [ ] `cargo test -p swissarmyhammer-code-context` passes
- [ ] `cargo test -p swissarmyhammer-tools` passes
- [ ] Manual: restart MCP, check `get status` after 5 seconds shows partial progress