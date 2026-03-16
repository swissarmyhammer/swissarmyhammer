---
position_column: done
position_ordinal: t0
title: 'CODE-CONTEXT-FIX-2: Complete tree-sitter write loop (finish TODO at mod.rs:652-663)'
---
Implement the missing code that makes tree-sitter parsing results persist to database.

**Current state:** mod.rs lines 622-669 has index_discovered_files_async() that:
1. Opens CodeContextWorkspace ✅
2. Calls ts_index.scan() to parse files ✅  
3. Extracts files from index ✅
4. **STOPS** - Has TODO comment but never writes anything ❌

**What needs to happen:**
Replace the TODO comment (lines 652-663) with actual implementation:
1. For each ParsedFile from ts_index.files()
2. Extract symbols using ensure_ts_symbols()
3. Generate call edges using generate_ts_call_edges()
4. Write chunks to ts_chunks table
5. Write edges to lsp_call_edges table (with source='treesitter')
6. Update indexed_files.ts_indexed = 1

**Why this is critical:** Tree-sitter parsing works but results are never saved. This is the direct cause of zero indexed files in `get status`.

**Quality Test Criteria:**
- cargo build succeeds
- Integration test on real project (this repo):
  - Run index_discovered_files_async() 
  - Query ts_chunks table - should have > 50k rows
  - Verify all .rs files from workspace are in indexed_files
  - Verify ts_indexed flag is set to 1
  - Run again - idempotent (no duplicates)