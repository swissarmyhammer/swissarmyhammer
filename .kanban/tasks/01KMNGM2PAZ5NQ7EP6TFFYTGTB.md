---
assignees:
- claude-code
depends_on:
- 01KMNGKRB4EBYE1RCD43P0JSE5
- 01KMNGK7SR2RXPZ0CES9D7C4CM
position_column: done
position_ordinal: ffffffffff8d80
title: 'shelltool-cli: init and deinit commands'
---
Wire up init/deinit in shelltool-cli using InitRegistry.\n\n## Registry components\n1. ShelltoolMcpRegistration (priority 10) — register shelltool MCP server via mirdan::mcp_config with command: \"shelltool\", args: [\"serve\"]\n2. ShellExecuteTool (priority 20) — config + deny Bash + deploy skill (from the real Initializable impl)\n\n## Commands\n- `shelltool init [project|local|user]` — runs registry init\n- `shelltool deinit [project|local|user]` — runs registry deinit (reverse order)\n\n## Files\n- shelltool-cli/src/main.rs (dispatch)\n- shelltool-cli/src/cli.rs (InstallTarget enum or reuse InitScope)\n\n## Acceptance\n- `shelltool init` registers MCP server, creates .shell/config.yaml, denies Bash, deploys shell skill\n- `shelltool deinit` reverses all\n- Idempotent