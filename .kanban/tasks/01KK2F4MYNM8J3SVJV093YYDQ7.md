---
position_column: done
position_ordinal: s6
title: Fix test_client_call_tool in swissarmyhammer-tools (cwd not found)
---
Test at swissarmyhammer-tools/src/mcp/test_utils.rs:143 fails with 'Failed to get current directory: No such file or directory'. The test environment's working directory doesn't exist when the test runs. #test-failure