---
position_column: done
position_ordinal: v4
title: 'CODE-CONTEXT-DB: Implement full startup_cleanup() with file reconciliation'
---
Implement startup_cleanup() fully to reconcile database with filesystem.

**Database schema:** ✅ Already implemented in FIX-1

**What still needs to happen:**
Implement startup_cleanup() function that:
- Walk filesystem with MD5 hashing (parallel via rayon)
- Diff against DB  
- Delete stale entries from indexed_files
- Mark changed files dirty (clear ts_indexed/lsp_indexed flags)
- Upsert current file set with last_seen_at timestamp
- Keep indexed_files table as source of truth

**Quality Test Criteria:**
1. Integration test on real project:
   - startup_cleanup() discovers all files
   - File modifications detected and marked dirty
   - File deletions removed from DB
   - New files added to DB
   - Second run is idempotent (no duplicates)
2. Edge cases:
   - Large projects (10k+ files)
   - Rapid file changes
   - Permissions issues