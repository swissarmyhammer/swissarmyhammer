---
position_column: done
position_ordinal: c8
title: 'Fix test_get_lines_retrieves_output - ''Failed to initialize shell state: No such file or directory'''
---
Test in swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs:4546 panics with ErrorCode(-32603) "Failed to initialize shell state: No such file or directory (os error 2)". Shell state initialization fails due to missing file/directory.