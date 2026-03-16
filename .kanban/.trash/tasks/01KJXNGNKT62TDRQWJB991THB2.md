---
position_column: done
position_ordinal: h0
title: 'Fix test: test_all_registered_tools_pass_cli_validation (swissarmyhammer-cli)'
---
Test panics at swissarmyhammer-cli/src/mcp_integration.rs:376 with 'Failed to create CliToolContext: Os { code: 2, kind: NotFound, message: "No such file or directory" }' -- likely missing a binary or fixture file needed for CLI tool validation. #test-failure