---
position_column: todo
position_ordinal: f0
title: 'Fix test_auto_diff_multiple_files_changed: ''Should report at least 2 files'' at line 337'
---
Test in swissarmyhammer-tools/tests/git_tool_integration_test.rs:337 panics with 'Should report at least 2 files'. The auto diff does not detect multiple changed files.