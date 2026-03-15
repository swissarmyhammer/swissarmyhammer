---
position_column: done
position_ordinal: e8
title: Fix test_grep_history_regex_pattern - regex grep should succeed
---
Test at swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs:4520 panics with: regex grep should succeed. Logs show Failed to store stdout/stderr for command: No such file or directory (os error 2). #test-failure