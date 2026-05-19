---
assignees:
- claude-code
position_column: todo
position_ordinal: '9680'
project: ai-panel
title: 'AI panel: pass the per-board MCP server to the spawned Claude CLI'
---
## What

When chatting in the AI panel, the `kanban` tool is not anchored to the open board's directory — it resolves `.kanban` against some other working directory. Root cause traced through the agent stack:

`crates/claude-agent/src/agent.rs` — `spawn_claude_for_new_session` builds the `claude` CLI's `SpawnConfig` with the agent's **static, build-time** MCP config:

```rust
let spawn_config = SpawnConfig::builder()
    ...
    .cwd(request.cwd.clone())
    .mcp_servers(self.config.mcp_servers.clone())   // <-- static config, NOT the session's servers
    ...
```

The per-board SAH MCP server URL is delivered correctly in the ACP `newSession.mcpServers` (`request.mcp_servers`). `connect_new_session_mcp_servers` connects it into the agent's own `mcp_manager`, but `spawn_claude_for_new_session` **never passes `request.mcp_servers` to the spawned `claude` CLI** — it passes `self.config.mcp_servers`.

In the kanban app, `apps/kanban-app/src/ai/agent_ws.rs` builds the agent via `create_agent(&model_config, None)` — `None` for `mcp_config` — so `self.config.mcp_servers` is **empty**. Consequence chain:

- `SpawnConfig.mcp_servers` is empty.
- `configure_mcp_servers` (`crates/claude-agent/src/claude_process.rs:430`) sees an empty list and returns early — the `claude` CLI is spawned with **no `--mcp-config` and no `--strict-mcp-config`**.
- The `claude` CLI then loads its **ambient** MCP configuration (the user's global `~/.claude.json` / project `.mcp.json`), picking up whatever `sah`/`kanban` server is configured there instead of the per-board HTTP MCP server.
- That ambient server is not rooted at the board folder, so the `kanban` tool operates on the wrong `.kanban`.

### Verified correct — do NOT change

- The per-board MCP server is correctly rooted: `apps/kanban-app/src/state.rs` `start_board_mcp_server` passes `Some(board_dir)` → `start_mcp_server_with_options` → `McpServer::new_with_work_dir` → `tool_context.working_dir`; the `kanban` tool's `get_kanban_context` reads `context.working_dir`. This path is fine.
- The webview is correct: `apps/kanban-app/ui/src/ai/acp-client.ts` sends `newSession({ cwd: boardDir, mcpServers: [boardMcpUrl] })`.

The bug is solely that the per-session MCP server never reaches the spawned `claude` CLI.

## Approach

In `crates/claude-agent/src/agent.rs`, `spawn_claude_for_new_session`: assemble `SpawnConfig.mcp_servers` from the **union** of `self.config.mcp_servers` and the per-session servers converted from `request.mcp_servers` via the existing `convert_acp_to_internal_mcp_config` (already used by `connect_new_session_mcp_servers` and `validate_new_session_mcp_config`).

With a non-empty list, `configure_mcp_servers` writes the temp `--mcp-config` file and adds `--strict-mcp-config` (`claude_process.rs:498-499`) — so the `claude` CLI connects to exactly the per-board HTTP MCP server and ignores ambient global/project MCP config.

To make this unit-testable, extract the `SpawnConfig` assembly into a pure helper on the agent — e.g. `fn build_session_spawn_config(&self, session_id, protocol_session_id, request) -> SpawnConfig` — and have `spawn_claude_for_new_session` call it. `spawn_claude_for_new_session` itself spawns a subprocess and is not unit-testable; the helper is.

Leave `connect_new_session_mcp_servers` / `mcp_manager` as-is — `mcp_manager` may serve the agent's own tool inventory; removing it is out of scope for this fix.

## Acceptance Criteria
- [ ] `build_session_spawn_config` (the extracted helper) returns a `SpawnConfig` whose `mcp_servers` includes every HTTP server from `request.mcp_servers`, in addition to any `self.config.mcp_servers`.
- [ ] With a session that carries an HTTP MCP server, the spawned `claude` CLI receives `--mcp-config <file>` and `--strict-mcp-config`, and the written config's `mcpServers` map contains the per-board server URL.
- [ ] `self.config.mcp_servers` entries are still included (a configured static server is not dropped).
- [ ] No change to `start_board_mcp_server`, `McpServer` rooting, or the webview ACP client.

## Tests
- [ ] In `crates/claude-agent/src/agent.rs` tests: build a `NewSessionRequest` with one HTTP `McpServer` and assert `build_session_spawn_config(...)` yields a `SpawnConfig` whose `mcp_servers` contains that server (converted to the internal HTTP variant).
- [ ] Add a test that with both a static `self.config.mcp_servers` entry and a session entry, both appear in the result.
- [ ] In `crates/claude-agent/src/claude_process.rs` `mod tests` (line ~802): assert `build_mcp_servers_map` on a non-empty HTTP server list produces the expected `mcpServers` entry; assert `configure_mcp_servers` on a non-empty list writes a config file and the `Command` gets `--mcp-config` and `--strict-mcp-config` (and on an empty list adds neither).
- [ ] Run `cargo test -p claude-agent` — all green.
- [ ] Run `cargo clippy -p claude-agent -- -D warnings` — clean.

## Workflow
- Use `/tdd` — write the failing `build_session_spawn_config` test first, then extract the helper and apply the union fix.
