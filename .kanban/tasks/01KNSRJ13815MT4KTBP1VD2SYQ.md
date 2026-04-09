---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffb880
title: Add tests for layered_context.rs LSP request methods
---
layered_context.rs (LSP methods)\n\nCoverage: 56.8% (187/329 lines)\n\nUncovered LSP-facing methods that require a mock RPC client:\n\n1. `lsp_request` (lines 191-209) - uncovered lines 202-205\n   Test: With mock LSP client, call lsp_request, verify response returned\n\n2. `lsp_notify` (lines 212-225) - uncovered lines 215-224\n   Test: With mock client, verify notification sent without error\n\n3. `lsp_request_with_document` (lines 240-294) - 28 uncovered lines\n   Test: Verify didOpen sent, request made, didClose sent (even on error), response returned; Ok(None) when no client\n\n4. `lsp_multi_request_with_document` (lines 308-357) - 23 uncovered lines\n   Test: Verify closure called with RPC handle; didOpen/didClose bookend it; Ok(None) when no client\n\nAll require injecting a SharedLspClient with a connected mock RpcClient.\n\n#coverage-gap #coverage-gap