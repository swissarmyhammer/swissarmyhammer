---
position_column: done
position_ordinal: f7
title: 'Fix shell execute test: test_get_lines_nonexistent_command'
---
Test panics at swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs:4602 with 'get lines for missing command should succeed with empty'. The grep/get_lines/list_processes operations fail with 'Failed to initialize shell state: No such file or directory (os error 2)' after a successful command execution. All 11 shell history tests share the same root cause. #test-failure