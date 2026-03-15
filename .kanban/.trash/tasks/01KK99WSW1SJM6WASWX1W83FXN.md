---
position_column: done
position_ordinal: v2
title: 'CODE-CONTEXT-3: Collect LSP symbol results and write to code-context DB'
---
After LSP server responds with symbols, persist them to code-context database.

**Requirements:**
- Handle LSP textDocument/definition requests
- Parse LSP response format (symbol locations, kinds)
- Convert to code-context FlatSymbol format
- Write using write_symbols() from code-context
- Write LSP call edges using write_edges()
- Mark file as lsp_indexed in DB
- Handle errors gracefully (missing symbols, timeout)

**Quality Test Criteria:**
1. Build succeeds: `cargo build 2>&1 | grep -c "error"` = 0
2. Real project test passes: `cargo test --test code_context_real_scenario_test 2>&1 | grep "test result"` contains "ok"
3. Concrete metrics on real project after LSP indexing:
   - lsp_indexed_files > 0 (currently 0, should be > 100 files minimum)
   - lsp_indexed_percent > 0 (currently 0.0)
   - lsp_symbol_count > 0 (currently 0, should be > 1k symbols from Rust codebase)
   - call_edge_count increases (from tree-sitter baseline, add LSP edges)
4. Spot-check symbols written: Query DB to verify at least 50 Rust symbols (functions, types) exist in lsp_symbols table
5. Error handling test: Verify graceful timeout (no crash) if LSP unresponsive for 30 seconds
6. All MCP integration tests pass: `cargo test --test code_context_mcp_e2e_test` — 4 tests, all pass