---
position_column: done
position_ordinal: a0
title: 'CODE-CONTEXT-CHUNKS: Persist tree-sitter chunks to database'
---
## Problem
Tree-sitter indexing worker marks files as indexed (1,008/16,577 files, 6.08%) but chunks are never written to database. This blocks all symbol-based operations (search, get_symbol, grep_code, etc.) from returning meaningful results.

## Current Status
- indexing.rs: run_indexing_worker() validates files exist and marks ts_indexed=1
- ts_chunks table in database remains empty (0 rows)
- No symbol data is persisted despite parsing completing
- Memory note: "Tree-sitter parsing completes but results never written (TODO at mod.rs:652-663)"

## Root Cause
Lines 88-90 of indexing.rs have comment saying "The treesitter module handles actual tree-sitter parsing" but the code (lines 91-104) only validates files and marks them indexed. Missing:
- Actual tree-sitter parsing via IndexContext::refresh()
- Chunk extraction via chunk_file()
- Database writes to ts_chunks table

## Solution
1. Use swissarmyhammer_treesitter::IndexContext to parse individual files
2. Extract chunks using chunk_file() for each parsed file
3. Write SemanticChunk data to ts_chunks table
4. Update file's ts_indexed flag only after chunks are written
5. Add comprehensive unit tests for chunk persistence

## Implementation Plan
- Modify run_indexing_worker() to parse files and write chunks
- Create helper function write_ts_chunks() to persist chunks
- Add tests:
  - test_parse_single_file_and_persist_chunks()
  - test_batch_parsing_writes_all_chunks()
  - test_chunks_have_correct_structure()
  - test_malformed_files_handled_gracefully()

## Expected Outcome
After fix, code context status should show:
- ts_chunk_count > 0
- Symbol lookups return actual results
- grep_code finds content in code chunks