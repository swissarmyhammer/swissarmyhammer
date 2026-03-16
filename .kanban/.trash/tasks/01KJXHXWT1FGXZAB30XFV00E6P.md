---
position_column: done
position_ordinal: e4
title: Fix test_get_lines_shows_line_numbers - No such file or directory (os error 2)
---
Test at swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs:4621 unwrap fails with ErrorData No such file or directory (os error 2). Same root cause as other get_lines tests - output storage directory missing. #test-failure