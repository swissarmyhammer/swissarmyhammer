---
position_column: todo
position_ordinal: e9
title: 'Fix test_auto_diff_clean_repo_returns_no_changes: assertion left==3, right==0 at line 273'
---
Test in swissarmyhammer-tools/tests/git_tool_integration_test.rs:273 fails with assertion `left == right` (left: 3, right: 0). A clean repo reports 3 changes instead of 0.