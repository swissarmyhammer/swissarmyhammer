---
position_column: done
position_ordinal: c6
title: Fix test_get_lines_nonexistent_command - panics with 'get lines for missing command should succeed with empty'
---
Test in swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs:4591 panics. The get_lines operation for a nonexistent command ID should return empty/success but instead fails.