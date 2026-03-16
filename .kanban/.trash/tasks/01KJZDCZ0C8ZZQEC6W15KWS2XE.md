---
position_column: done
position_ordinal: k3
title: 'Fix test_client_call_tool: MCP error from missing CWD'
---
Test in swissarmyhammer-tools/src/mcp/test_utils.rs:143 panics with McpError: "Failed to get current directory: No such file or directory (os error 2)". Same CWD race condition. #test-failure