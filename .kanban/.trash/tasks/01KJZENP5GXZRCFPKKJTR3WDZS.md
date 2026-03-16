---
position_column: done
position_ordinal: l1
title: Fix test_client_call_tool
---
File: swissarmyhammer-tools/src/mcp/test_utils.rs:143. Panic: unwrap on Err - McpError(ErrorData { code: ErrorCode(-32603), message: "Failed to get current directory: No such file or directory (os error 2)" }). The MCP test utility cannot resolve the current directory. #test-failure