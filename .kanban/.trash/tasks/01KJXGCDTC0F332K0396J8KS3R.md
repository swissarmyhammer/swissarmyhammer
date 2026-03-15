---
position_column: done
position_ordinal: c2
title: 'Fix test_client_call_tool: "Failed to get current directory: No such file or directory"'
---
Test mcp::test_utils::tests::test_client_call_tool panics with unwrap on Err: McpError "Failed to get current directory: No such file or directory (os error 2)". The test environment CWD is missing. File: /Users/wballard/github/swissarmyhammer/swissarmyhammer-tools/swissarmyhammer-tools/src/mcp/test_utils.rs:143