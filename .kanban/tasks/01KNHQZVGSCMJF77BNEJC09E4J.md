---
assignees:
- claude-code
position_column: todo
position_ordinal: ba80
title: 'Coverage: indexing.rs — run_indexing_worker orchestration loop'
---
swissarmyhammer-code-context/src/indexing.rs

Coverage: 68.4% (54/79)

The individual component functions (query_dirty_files, mark_ts_indexed, write_ts_chunks, parse_and_extract_chunks) are well-tested. The gap is the `run_indexing_worker` orchestration loop and `spawn_indexing_worker` which tie them together.

Uncovered paths in `run_indexing_worker`:
1. The rayon parallel map branch where `full_path.exists()` is false (line 100-102) — the worker logs a warning and returns empty chunks. This is partially covered by test_worker_nonexistent_file_is_marked_indexed but that test manually simulates the steps rather than running the actual loop.
2. The `parse_and_extract_chunks` error branch (line 111-113) — when parsing fails, the worker returns empty chunks.
3. The `write_ts_chunks` error + fallback `mark_ts_indexed` path (lines 132-142) — when chunk write fails, the worker still marks indexed. The test_worker_chunk_write_failure_still_marks_indexed test simulates this but doesn't run the actual loop.
4. The `mark_ts_indexed` error after successful chunk write (line 144-146).

`spawn_indexing_worker` is completely untested (it just spawns a thread that runs the infinite loop).

What to test:
- Extract the per-batch processing logic from run_indexing_worker into a testable `process_dirty_batch` function that takes workspace_root, db, and config. This avoids the infinite loop problem.
- Test the extracted function with: a mix of existing and non-existing files, files that fail to parse (binary content), files that fail chunk write (corrupted DB mid-batch).
- Alternatively, add a `max_iterations` field to IndexingConfig and test the full loop with max_iterations=1.

#coverage-gap #code-context