---
title: 'Fix hanging test: integration::flow_mcp_integration::test_flow_execution_via_mcp'
position:
  column: todo
  ordinal: c0
---
The test `integration::flow_mcp_integration::test_flow_execution_via_mcp` in swissarmyhammer-tools hangs indefinitely (observed running for over 280 seconds with no progress). It appears to be stuck in an infinite wait, likely a deadlock or missing timeout. All other 693 tests in the package pass. File: swissarmyhammer-tools/tests/ (integration test in flow_mcp_integration module).