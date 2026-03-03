---
title: 'Fix test_files_read_tool_integration: Unknown tool "files"'
position:
  column: done
  ordinal: d3
---
Test `integration::cli_mcp_integration::test_files_read_tool_integration` in `swissarmyhammer-cli` fails with: `ErrorData { code: ErrorCode(-32600), message: "Unknown tool: files", data: None }`. The test at `/Users/wballard/github/swissarmyhammer/swissarmyhammer/swissarmyhammer-cli/tests/integration/cli_mcp_integration.rs:84` attempts to execute a tool named "files" which is not registered. The tool may have been renamed or removed. #test-failure