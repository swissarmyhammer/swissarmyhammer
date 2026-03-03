---
position_column: done
position_ordinal: f3
title: 'Fix hanging test: integration::flow_mcp_integration::test_flow_execution_via_mcp'
---
**Resolution:** Already fixed. The test now has a 2-second timeout via `try_execute_workflow` that wraps the workflow call. Test passes in ~7 seconds (including compilation). The `example-actions` workflow times out as expected and the error is handled gracefully. No hang.\n\nThe original hang was likely before the timeout was added to `try_execute_workflow`.