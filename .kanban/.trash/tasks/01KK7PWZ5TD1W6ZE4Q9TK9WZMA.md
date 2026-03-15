---
position_column: todo
position_ordinal: e8
title: 'Fix test_file_diff_disk_to_disk: assertion left==0, right==1 at line 109'
---
Test in swissarmyhammer-tools/tests/git_tool_integration_test.rs:109 fails with assertion `left == right` (left: 0, right: 1). The diff result has 0 entities but expected 1.