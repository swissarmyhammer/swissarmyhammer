---
position_column: done
position_ordinal: u9
title: 'CODE-CONTEXT-QUERIES: Verify grep/search/query operations return real data'
---
Verify that grep_code, search_code, and query_ast operations work correctly with indexed database.

**Status:** Operations already implemented and dispatch correctly ✅

**What needs testing:**
Now that database is populated with tree-sitter data, verify:
- grep_code: Ripgrep-powered keyword search in ts_chunks text
  - Pattern "pub fn" finds ≥100 functions
  - Language filter "rust" excludes non-Rust matches
  - Results include full source text (not fragments)
  
- search_code: Semantic similarity via embeddings
  - Semantic query "authentication" finds auth-related code
  - Top_k parameter limits results correctly
  - min_similarity threshold works

- query_ast: Tree-sitter S-expression query
  - S-expr for function_item returns AST nodes with ranges

**Quality Test Criteria:**
1. Integration test on swissarmyhammer-tools repo:
   - grep_code returns results when ts_chunks populated
   - Result count matches indexed functions
   - Source text is complete
2. All blocking during indexing works correctly
3. get_status exception (always returns immediately) works