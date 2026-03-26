---
assignees:
- claude-code
depends_on: []
position_column: done
position_ordinal: ffffffc280
title: Wire ValidatorMcpServer into AvpContext agent creation
---
## What\nConnect the `/mcp/validator` endpoint to the validator agent and lock down its tool access.\n\n**Files:**\n- `avp-common/src/context.rs` — `AvpContext::agent()` method\n- `swissarmyhammer-agent/src/lib.rs` — `CreateAgentOptions`, `create_agent_with_options`\n\n**Approach:**\nThe existing code passes `mcp_config: None` and no tools override. Change to:\n1. Add `tools_override: Option<String>` to `CreateAgentOptions`\n2. Construct `McpServerConfig` pointing at `http://localhost:{port}/mcp/validator`\n3. Pass `tools_override: Some(\"\".to_string())` to disable built-in tools\n4. Thread `tools_override` through to `AgentConfig.claude.tools_override`\n\nThe MCP port is already known (the SAH server is already running). No new server needed.\n\nResult: `--tools \"\" --mcp-config <path> --strict-mcp-config` where the config points at `/mcp/validator`. The validator has ONLY code_context + files (read-only).\n\n## Acceptance Criteria\n- [ ] Validator agent gets MCP config pointing at `/mcp/validator`\n- [ ] `--tools \"\"` disables built-in tools\n- [ ] `--strict-mcp-config` limits to our MCP config only\n- [ ] Validator can call code_context and read files\n- [ ] Validator CANNOT call shell, write, edit, kanban, etc.\n\n## Tests\n- [ ] Unit test: CreateAgentOptions with tools_override produces correct config\n- [ ] Integration test: validator reads a file via MCP\n- [ ] `cargo test -p avp-common`"
