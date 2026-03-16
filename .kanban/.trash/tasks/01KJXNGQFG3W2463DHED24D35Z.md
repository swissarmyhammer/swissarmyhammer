---
position_column: done
position_ordinal: h1
title: 'Fix test: test_cli_tool_context_creation (swissarmyhammer-cli)'
---
Test panics at swissarmyhammer-cli/src/mcp_integration.rs:292 with 'Failed to create CliToolContext: Some(Os { code: 2, kind: NotFound, message: "No such file or directory" })' -- same root cause as test_all_registered_tools_pass_cli_validation, missing file/binary dependency. #test-failure