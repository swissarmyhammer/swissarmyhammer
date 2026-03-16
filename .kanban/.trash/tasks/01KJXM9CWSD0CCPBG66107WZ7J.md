---
position_column: done
position_ordinal: f6
title: 'Fix swissarmyhammer-tools integration tests: MCP server and file path validator failures'
---
Several integration tests in swissarmyhammer-tools fail in full workspace runs due to CWD poisoning from the INITIAL_CWD Lazy static. Affected tests include: test_mcp_server_list_prompts, test_mcp_server_uses_same_directory_discovery, test_reload_prompts_detects_deleted_prompts, test_reload_prompts_detects_new_prompts, test_reload_prompts_detects_no_changes, test_mcp_server_uses_same_prompt_paths_as_cli, test_file_path_validator_relative_paths, test_file_path_validator_relative_with_workspace, test_secure_file_access_read_relative_paths, test_validate_file_path_relative, test_load_all_questions. These are all cascading from the same INITIAL_CWD poisoning root cause.