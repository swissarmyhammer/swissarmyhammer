---
position_column: done
position_ordinal: e0
title: 'Fix test: test_cli_tool_context_creation'
---
Test in swissarmyhammer-cli/src/mcp_integration.rs:291 panics with: Failed to create CliToolContext: Some(Os { code: 2, kind: NotFound, message: "No such file or directory" }). Same root cause as test_all_registered_tools_pass_cli_validation -- missing file or binary. #test-failure