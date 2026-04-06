---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff9080
title: 'Test tool_registry: CLI integration, validation, and disabled-tool filtering'
---
File: swissarmyhammer-tools/src/mcp/tool_registry.rs (65.7%, 107 uncovered lines)

Uncovered areas:
- ToolContext builder methods: with_prompt_library, with_plan_sender, with_peer, with_tool_registry, with_working_dir, set_mcp_server
- call_tool() - the main tool dispatch method
- get_cli_categories() / get_tools_for_category() / get_cli_tools()
- validate_cli_tools() / validate_all_tools() / validate_tool()
- validate_schema() / validate_cli_requirements_for_tool()
- ToolValidationReport methods
- send_mcp_log()
- register_file_tools() / create_fully_registered_tool_registry()

Also shared_utils.rs (58.3%, 40 uncovered lines):
- validate_ulid(), format_timestamp(), format_file_size(), format_list_summary()
- handle_error() / handle_result()

Also utils.rs (0%, 18 uncovered lines) - entirely untested utility functions. #coverage-gap