---
position_column: done
position_ordinal: f0
title: Fix test_grep_history_with_limit - grep with limit should succeed
---
Test at swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs:4498 panics with: grep with limit should succeed. Same root cause - Failed to store stdout for command: No such file or directory (os error 2). #test-failure