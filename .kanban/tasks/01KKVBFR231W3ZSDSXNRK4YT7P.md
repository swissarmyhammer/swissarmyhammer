---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffff9780
title: Missing integration test for /mcp/validator endpoint
---
**swissarmyhammer-tools/src/mcp/unified_server.rs:create_mcp_router()**\n\nThe unit tests cover `FilesTool::read_only()` behavior well, but there's no integration test verifying that:\n1. `/mcp/validator` returns exactly 2 tools (`files` and `code_context`)\n2. `/mcp` still returns the full tool set\n3. A validator agent CANNOT reach disallowed tools\n\n**Why this matters:** The plan's verification section explicitly calls for these tests. Without them, route ordering bugs or axum nesting issues could silently break the lockdown.\n\n**Fix:** Add an integration test in `unified_server.rs::tests` that starts an HTTP server, hits both endpoints, and asserts tool counts.\n\n**Verification:** Test exists and passes.