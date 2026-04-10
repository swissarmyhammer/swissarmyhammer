---
assignees:
- claude-code
position_column: todo
position_ordinal: '8480'
title: Add tests for LspDaemon health check failure paths
---
daemon.rs:272, 315-323\n\nCoverage: 74.6% (153/205 lines)\n\nUncovered lines: 272, 315-318, 321-323\n\nFunction: `is_healthy()` — two failure paths:\n1. Line 272: `child.try_wait()` but no child exists (early return false)\n2. Lines 315-318: process exited unexpectedly (Ok(Some(status))) — clears client, records failure\n3. Lines 321-323: try_wait error (Err(e)) — clears client, records failure\n\nTest scenarios:\n- Call is_healthy when no child process → false\n- Process exited with status → state transition, client cleared\n- try_wait returns Err → state transition, client cleared\n\n#coverage-gap #coverage-gap