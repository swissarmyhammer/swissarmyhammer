---
position_column: done
position_ordinal: t4
title: 'CODE-CONTEXT-LEADER: Implement leader/reader process coordination'
---
Implement flock-based leader election and multi-process database coordination per spec lines 137-150.

**Requirements:**
- Use .code-context/leader.lock for flock-based leader election
- Leader: acquires lock, runs startup_cleanup, manages watchers, writes to DB
- Readers: open DB read-only, query results, never write
- Integrate with existing swissarmyhammer-leader-election crate
- Leader holds file watcher (not readers)
- Readers gracefully degrade if no leader (use stale data)

**Quality Test Criteria:**
1. Build succeeds
2. Unit test: leader election works, first process gets lock
3. Unit test: readers can't acquire lock, open DB in read mode
4. Integration test on real project:
   - One leader process starts, acquires lock
   - Multiple reader processes open DB read-only (no error)
   - Leader runs startup_cleanup
   - Readers immediately see indexed_files updates
   - Leader shutdown releases lock
   - New process becomes leader