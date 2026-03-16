---
position_column: done
position_ordinal: d9
title: 'Fix test: test_all_registered_tools_pass_cli_validation'
---
Test in swissarmyhammer-cli/src/mcp_integration.rs:374 panics with: Failed to create CliToolContext: Os { code: 2, kind: NotFound, message: "No such file or directory" }. The test cannot find a required file or binary when creating CliToolContext. #test-failure