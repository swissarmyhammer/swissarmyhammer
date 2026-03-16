---
position_column: done
position_ordinal: k4
title: 'Fix 12 shell execute tests: "Failed to initialize shell state" from CWD race'
---
Tests in swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs all fail with "Failed to initialize shell state: No such file or directory (os error 2)". Affected tests: test_get_lines_nonexistent_command (line 4591), test_grep_history_no_matches (line 4448), test_get_lines_shows_line_numbers (line 4609), test_get_lines_retrieves_output (line 4546), test_get_lines_with_range (line 4575), test_grep_history_finds_matching_output (line 4431), test_grep_history_regex_pattern (line 4512), test_list_processes_table_format (line 4346), test_list_processes_shows_completed_commands (line 4313), test_grep_history_with_limit (line 4491), test_grep_history_with_command_id_filter (line 4470). Same CWD race condition as other failures. #test-failure