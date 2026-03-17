---
assignees:
- claude-code
depends_on:
- 01KKSXHGQN1FFY1M8AQC3AT6B1
position_column: todo
position_ordinal: '8180'
title: Create ValidatorMcpServer — minimal MCP server for validator agents
---
## What\nCreate a `ValidatorMcpServer` in `avp-common/src/validator/mcp_server.rs` that starts a minimal HTTP MCP server exposing only two tools: `code_context` and `validator_files` (read-only).\n\nThis server runs on a random port, managed by the `AvpContext`. It uses the existing `McpServer` infrastructure from `swissarmyhammer-tools` but registers only the two tools.\n\n**Files:**\n- `avp-common/src/validator/mcp_server.rs` (new)\n- `avp-common/src/validator/mod.rs` (add module)\n\n**Approach:**\n- Reuse `McpServer` from swissarmyhammer-tools but create it with a custom registration function that only registers `code_context` + `validator_files`\n- Start it on a random port (like the existing side HTTP server does)\n- Return the port so callers can construct the MCP URL\n- The server shares the existing code-context index (same `.code-context/` database)\n\n## Acceptance Criteria\n- [ ] `ValidatorMcpServer::start(work_dir) -> Result<(u16, JoinHandle)>` starts and returns port\n- [ ] Only `code_context` and `validator_files` tools are registered\n- [ ] Server is accessible at `http://localhost:{port}/mcp`\n- [ ] Health endpoint responds at `http://localhost:{port}/health`\n- [ ] Shares existing code-context index\n\n## Tests\n- [ ] Integration test: start server, list tools, verify only 2 tools present\n- [ ] Integration test: call code_context detect projects through the server\n- [ ] `cargo test -p avp-common`