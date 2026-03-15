---
position_column: done
position_ordinal: v0
title: 'CODE-CONTEXT-DUPLICATES: Verify call graph and blast radius operations'
---
Verify that call graph and blast radius operations work correctly with indexed call edges.

**Status:** Operations already implemented and dispatch correctly ✅

**What needs testing:**
Now that database is populated with call edges from tree-sitter and LSP, verify:
- get_callgraph: Traverse call graph from a symbol
  - Inbound (callers) traversal works
  - Outbound (callees) traversal works
  - Both directions work
  - Max_depth parameter respected
  - Returns correct edge count
  
- get_blastradius: Analyze impact of changes
  - File-level blast radius works
  - Symbol-level blast radius works
  - Max_hops parameter respected
  - Impact organized by hop distance

**Quality Test Criteria:**
1. Integration test on swissarmyhammer-tools repo:
   - get_callgraph finds inbound callers (≥5 for common functions)
   - Outbound callees listed correctly
   - Depth=1 finds immediate neighbors
   - Depth=2 includes transitive calls
2. Blast radius:
   - File changes show 1-hop and 2-hop impacts
   - Symbol-level shows affected symbols
3. Call edges correctly sourced (lsp/treesitter)
4. No infinite loops on circular dependencies