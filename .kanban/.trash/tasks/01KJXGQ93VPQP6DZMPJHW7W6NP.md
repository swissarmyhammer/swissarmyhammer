---
position_column: done
position_ordinal: c7
title: Fix test_grep_history_no_matches - panics with 'grep with no matches should succeed'
---
Test in swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs:4448 panics. grep_history with no matching results should succeed but fails.