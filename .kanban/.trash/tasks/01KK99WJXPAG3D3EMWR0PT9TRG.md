---
position_column: done
position_ordinal: u4
title: 'CODE-CONTEXT-1: Extract IndexContext results to code-context DB schema'
---
CRITICAL PATH BLOCKER
After IndexContext.scan() completes, extract ParsedFile objects and write symbols to code-context SQLite DB.

**Requirements:**
- For each file from ts_index.files(), get the ParsedFile
- Extract symbols (function/struct/impl definitions)
- Write using ensure_ts_symbols() from code-context
- Write call edges using write_ts_edges()
- Mark file as ts_indexed in DB

**Quality Test Criteria:**
1. Build succeeds: `cargo build 2>&1 | grep -c "error"` = 0
2. Real project test passes: `cargo test --test code_context_real_scenario_test 2>&1 | grep "test result"` contains "ok"
3. Concrete metrics on real project:
   - ts_indexed_files > 0 (currently 0, should be > 16k files)
   - ts_indexed_percent > 0 (currently 0.0)
   - ts_chunk_count > 0 (currently 0, should be ~50k-100k chunks)
   - call_edge_count > 0 (currently 0, should be > 1k edges)
4. Spot-check symbols written: Query DB directly to verify at least 100 symbols exist in ts_symbols table for swissarmyhammer-tools project
5. All MCP integration tests pass: `cargo test --test code_context_mcp_e2e_test` — 4 tests, all pass