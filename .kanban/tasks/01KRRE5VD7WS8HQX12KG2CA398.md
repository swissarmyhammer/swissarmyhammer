---
assignees:
- claude-code
depends_on:
- 01KRRE4PJV0N1GSE92MF45GGPV
position_column: todo
position_ordinal: '8880'
project: plugin-arch
title: 'plugin: CliServer stdio subprocess transport'
---
## What
Implement `CliServer` — an `McpServer` backed by a spawned subprocess that speaks MCP JSON-RPC over its stdio.

In `crates/swissarmyhammer-plugin/src/server/cli.rs`:
- Constructed from `{ cli: Vec<String>, env: Option<Map>, cwd: Option<PathBuf> }` (the CLI `ServerSource` variant).
- On construction/connect: spawn the subprocess, perform the MCP `initialize` handshake over stdin/stdout, send `tools/list`, cache the response as `Vec<ToolMetadata>`. Subscribe to `notifications/tools/list_changed` to refresh the cache.
- `invoke(caller, tool, input)` → send a JSON-RPC `tools/call` (tool name + arguments map, unchanged) over stdin, await the matching response on stdout, return the result `Value`. Errors map to platform `Error`.
- Manage subprocess lifecycle: kill on drop/unregister; restart-on-crash policy (at minimum, surface a crash and fail subsequent calls cleanly — full auto-restart can be minimal).
- **Build on the rmcp client SDK** — use rmcp's client-side stdio/child-process transport as the foundation; hand-roll JSON-RPC framing only for gaps rmcp does not cover. Stay consistent with the rmcp-everywhere principle.

## Acceptance Criteria
- [ ] `CliServer` exists, implements `McpServer`, spawns the subprocess and completes the MCP handshake via the rmcp client.
- [ ] `tools()` reflects the subprocess's `tools/list`; `invoke` round-trips a `tools/call` over stdio.
- [ ] The subprocess is killed when the `CliServer` is dropped/unregistered.

## Tests
- [ ] Integration test in `swissarmyhammer-plugin/tests/`: use a real, tiny MCP stdio server as the subprocess — the `files` tool exposed as a standalone stdio binary, or a minimal fixture MCP server built in the test crate. Register it as a `CliServer`, call a tool, assert the result.
- [ ] Test that dropping the `CliServer` terminates the child process (assert the PID is gone).
- [ ] Run: `cargo test -p swissarmyhammer-plugin` — all green.

## Workflow
- Use `/tdd` — write the subprocess round-trip test against a real stdio MCP server first, then implement.

## Depends on
McpServer trait + ToolMetadata.