---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffc680
title: Add tests for workspace follower retry and try_promote
---
workspace.rs\n\nCoverage: 76.3% (58/76 lines)\n\nUncovered lines: 52, 102, 106, 110, 128, 141, 143-144, 151, 154, 180, 217, 223, 257-260, 271\n\nThree areas:\n1. Follower retry loop (lines 128-154) - retries waiting for DB file, backoff, exhaustion after >10 retries\n2. `try_promote` (lines 204-236) - promotes follower to leader: opens RW connection, runs startup_cleanup, transitions mode\n3. `DbRef::Owned` deref (line 271) + Debug impl (lines 257-260)\n\nTest scenarios:\n- Open as follower before leader creates DB → verify retry loop runs, eventually succeeds or errors\n- Drop leader, call try_promote on follower → verify returns Some(shared_db), is_leader() now true\n- DbRef::Owned path via follower .db() call\n- WorkspaceMode Debug impl\n\n#coverage-gap #coverage-gap