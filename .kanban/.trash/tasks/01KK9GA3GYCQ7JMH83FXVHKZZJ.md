---
position_column: done
position_ordinal: u7
title: 'CODE-CONTEXT-INIT: Implement startup indexing trigger for tree-sitter and LSP'
---
**Infrastructure Complete ✅ | Tree-sitter Integration Remaining**

**Status: In Progress**
- ✅ In-process parallel indexing worker thread created
- ✅ Spawned automatically when CodeContextWorkspace becomes leader
- ✅ Work queue pattern implemented (batch query → parallel process → DB update)
- ✅ Database schema understood and accessible
- ⏳ Tree-sitter parsing integration (placeholder currently just marks files done)

**Current Implementation:**
1. When leader elected: spawns indexing worker thread
2. Worker queries dirty files (ts_indexed=0) in batches of 100
3. Uses rayon for parallel processing (max 4 concurrent tasks)
4. PLACEHOLDER: Currently just marks files as indexed without parsing
5. Loops until no dirty files remain

**Still Needed:**
For each dirty file in parallel:
- Use swissarmyhammer_treesitter::IndexContext to parse file
- Extract chunks with chunk_file()
- Insert chunks into ts_chunks table
- Update ts_indexed=1 only after chunks written

This requires integrating treesitter crate (already a dependency) into the indexing loop.

**Quality Test Criteria:**
1. After `build_status`, running get_status shows progress (increasing ts_indexed_files)
2. Within 30 seconds: ts_indexed_files > 0
3. Within 5 minutes: ts_indexed_files ≥ 10,000
4. Indexing completes without hanging or crashing
5. Symbol operations work after indexing