---
position_column: done
position_ordinal: ffff9a80
title: Fix 9 failing shell execute tests (grep_history/get_lines) in swissarmyhammer-tools
---
Nine tests in swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs fail because shell history storage fails with 'No such file or directory'. The temp directory used for storing stdout/stderr doesn't exist. Affected: test_get_lines_nonexistent_command, test_get_lines_retrieves_output, test_get_lines_shows_line_numbers, test_get_lines_with_range, test_grep_history_finds_matching_output, test_grep_history_no_matches, test_grep_history_regex_pattern, test_grep_history_with_command_id_filter, test_grep_history_with_limit #test-failure