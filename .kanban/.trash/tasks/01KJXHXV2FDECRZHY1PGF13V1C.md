---
position_column: done
position_ordinal: e3
title: Fix test_get_lines_retrieves_output - No such file or directory (os error 2)
---
Test at swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs:4555 panics with: get lines should succeed: ErrorData No such file or directory (os error 2). The shell output storage directory does not exist when tests run. #test-failure