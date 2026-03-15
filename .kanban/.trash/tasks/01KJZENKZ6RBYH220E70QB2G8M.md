---
position_column: done
position_ordinal: l0
title: 'Fix shell execute tests (10 failures) - "Failed to initialize shell state: No such file or directory"'
---
File: swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs. 10 tests fail with "Failed to initialize shell state: No such file or directory (os error 2)". Affected tests: test_get_lines_nonexistent_command (line 4602), test_get_lines_retrieves_output (line 4555), test_get_lines_shows_line_numbers (line 4621), test_get_lines_with_range (line 4585), test_grep_history_finds_matching_output (line 4435), test_grep_history_no_matches (line 4453), test_grep_history_regex_pattern (line 4520), test_grep_history_with_command_id_filter (line 4476), test_grep_history_with_limit (line 4498), test_list_processes_shows_completed_commands (line 4315), test_list_processes_table_format (line 4349). The shell state initialization cannot find a required directory. #test-failure