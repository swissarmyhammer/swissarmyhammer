---
position_column: done
position_ordinal: v3
title: 'CODE-CONTEXT-4: End-to-end validation test on real project'
---
Validate that code_context tool actually works on swissarmyhammer-tools project.

**Requirements:**
- Run on actual swissarmyhammer-tools project (not isolated temp project)
- Verify get_status shows: total_files > 0, ts_indexed_files > 0, lsp_indexed_files > 0
- Test search_symbol returns actual results (not empty array)
- Test get_symbol returns source_text for found symbols
- Test grep_code finds actual patterns in code
- Test get_callgraph shows actual call edges

**Quality Test Criteria:**
1. Build succeeds: `cargo build 2>&1 | grep -c "error"` = 0
2. get_status on real project shows:
   - total_files ≥ 16,000
   - ts_indexed_files ≥ 10,000 (should index most Rust files)
   - lsp_indexed_files ≥ 100
   - ts_chunk_count ≥ 50,000
   - call_edge_count ≥ 1,000
3. search_symbol("CodeContextTool") returns ≥ 1 result
4. get_symbol("CodeContextTool::execute") returns source_text with function body (not empty)
5. grep_code("pub fn") finds ≥ 100 matches
6. get_callgraph("execute") in direction "inbound" shows ≥ 1 caller
7. Comprehensive integration test that validates all 6 operations in sequence on real project
8. All existing tests pass: `cargo test --test code_context_mcp_e2e_test`, `cargo test --test code_context_real_scenario_test`