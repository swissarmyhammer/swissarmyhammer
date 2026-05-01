---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffff8c80
title: 'WARNING: flush_changes holds cache write lock during directory scan I/O'
---
swissarmyhammer-store/src/handle.rs:662-726\n\nflush_changes() acquires the cache write lock at line 667 and holds it across the entire directory scan (tokio::fs::read_dir + read_to_string for every file). This means all concurrent write() and delete() calls are blocked for the entire duration of the scan.\n\nFor a store with many files (e.g. tasks directory with hundreds of task files), this could block writes for the time it takes to read all files from disk. The write lock is needed for cache replacement at line 723, but the scan itself only needs to read the old cache state.\n\nSuggestion: Split the operation into two phases: (1) snapshot the cache under a read lock, (2) scan the directory without any lock, (3) acquire the write lock only for the cache replacement. This matches the pattern already used in StoreContext::undo() where the stores lock is released before the async undo operation.",
<parameter name="tags">["review-finding"] #review-finding