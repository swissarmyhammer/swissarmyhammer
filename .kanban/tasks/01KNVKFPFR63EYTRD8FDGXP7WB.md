---
assignees:
- claude-code
position_column: todo
position_ordinal: '8580'
title: Add tests for LspDaemon restart_with_backoff and shutdown
---
daemon.rs:344-345, 395-396\n\nCoverage: 74.6% (153/205 lines)\n\nUncovered lines: 344-345, 395-396\n\nFunctions:\n1. `restart_with_backoff()` (lines 333-351) — lines 344-345 are the info log + sleep before restart\n2. `shutdown()` (lines 370-407) — lines 395-396 are the Ok(Err(e)) path: graceful shutdown returned error\n\nTest scenarios:\n- restart_with_backoff with low failure count → verify sleep + restart called\n- shutdown where graceful_shutdown returns Err → verify warning logged, state = NotStarted\n\n#coverage-gap #coverage-gap