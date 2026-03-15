---
position_column: done
position_ordinal: e6
title: 'Fix test_grep_history_finds_matching_output - Grep failed: No such file or directory'
---
Test at swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs:4435 panics with: grep should succeed: Grep failed: No such file or directory (os error 2). The grep history operation cannot find stored command output files. #test-failure