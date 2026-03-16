---
position_column: done
position_ordinal: d6
title: 'Fix test_list_processes_shows_completed_commands - ''Failed to initialize shell state: No such file or directory'''
---
Test in swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs:4313 panics with ErrorCode(-32603) "Failed to initialize shell state: No such file or directory (os error 2)".