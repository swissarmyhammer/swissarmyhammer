---
position_column: done
position_ordinal: c4
title: 'Fix shell execute tests: 11 tests fail with "Failed to initialize shell state: No such file or directory"'
---
11 tests in mcp::tools::shell::execute::tests fail. All share a common root cause: "Failed to initialize shell state: No such file or directory (os error 2)". Tests: test_get_lines_nonexistent_command, test_grep_history_no_matches, test_get_lines_retrieves_output, test_get_lines_shows_line_numbers, test_get_lines_with_range, test_grep_history_finds_matching_output, test_grep_history_regex_pattern, test_grep_history_with_command_id_filter, test_grep_history_with_limit, test_list_processes_shows_completed_commands, test_list_processes_table_format. File: /Users/wballard/github/swissarmyhammer/swissarmyhammer-tools/swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs