---
position_column: done
position_ordinal: u8
title: 'CODE-CONTEXT-SYMBOLS: Verify symbol operations return indexed data'
---
Verify that symbol operations work correctly with indexed database.

**Status:** Operations already implemented and dispatch correctly ✅

**What needs testing:**
Now that database is populated with LSP and tree-sitter symbol data, verify:
- get_symbol: Locate symbol by name with fuzzy matching
  - Exact match tier works
  - Suffix/prefix matching works
  - Case-insensitive matching works
  - Returns correct file paths and line numbers
  
- search_symbol: Fuzzy search across all symbols
  - Kind filter (function, struct, method, etc.) works
  - Max_results parameter respected
  - Relevance ranking correct
  
- list_symbols: List all symbols in a file
  - Returns all symbols in correct order
  - Includes correct metadata (kind, line, char)

**Quality Test Criteria:**
1. Integration test on swissarmyhammer-tools repo:
   - get_symbol finds known functions (e.g., "chunk_file", "parse")
   - search_symbol with kind="function" filters correctly
   - list_symbols on core files returns ≥50 symbols
   - Symbol locations are accurate
2. Merged LSP + tree-sitter results work
3. Symbol source provenance (lsp/treesitter/merged) is correct