---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff8380
title: Test indexing worker error recovery and chunking edge cases
---
File: swissarmyhammer-code-context/src/indexing.rs (67.1%, 26 uncovered lines)\n\nUncovered paths:\n- `spawn_indexing_worker()` error callback (lines 60-61)\n- `run_indexing_worker()`: file-not-found path (line 101), parse failure path (lines 111-113), chunk write failure with mark-indexed fallback (lines 132-141), mark-indexed failure (line 144), empty chunk skip-and-mark path (lines 121-129)\n- `parse_and_extract_chunks()` (lines 211-230): chunk boundary logic, empty file handling\n\nTests needed:\n- Unit test parse_and_extract_chunks with empty file, small file, multi-chunk file\n- Integration test with temp DB: insert dirty file, run worker, verify indexed\n- Error path: nonexistent file marked dirty\n\nAcceptance: coverage >= 80% for indexing.rs #coverage-gap