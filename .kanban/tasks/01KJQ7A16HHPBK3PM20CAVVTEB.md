---
title: 'Fix test_files_read_tool_integration: Unknown tool ''files'''
position:
  column: done
  ordinal: d2
---
Test `integration::cli_mcp_integration::test_files_read_tool_integration` in `swissarmyhammer-cli` fails with: ErrorData { code: ErrorCode(-32600), message: "Unknown tool: files", data: None }. The test at swissarmyhammer-cli/tests/integration/cli_mcp_integration.rs:84 expects a 'files' tool to be registered, but it is not found. The tool may have been renamed or removed. #test-failure