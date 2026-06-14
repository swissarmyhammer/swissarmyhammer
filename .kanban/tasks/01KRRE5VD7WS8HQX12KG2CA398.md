---
assignees:
- claude-code
depends_on:
- 01KRRE4PJV0N1GSE92MF45GGPV
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffff580
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
- [x] `CliServer` exists, implements `McpServer`, spawns the subprocess and completes the MCP handshake via the rmcp client.
- [x] `tools()` reflects the subprocess's `tools/list`; `invoke` round-trips a `tools/call` over stdio.
- [x] The subprocess is killed when the `CliServer` is dropped/unregistered.

## Tests
- [x] Integration test in `swissarmyhammer-plugin/tests/`: use a real, tiny MCP stdio server as the subprocess — the `files` tool exposed as a standalone stdio binary, or a minimal fixture MCP server built in the test crate. Register it as a `CliServer`, call a tool, assert the result.
- [x] Test that dropping the `CliServer` terminates the child process (assert the PID is gone).
- [x] Run: `cargo test -p swissarmyhammer-plugin` — all green.

## Workflow
- Use `/tdd` — write the subprocess round-trip test against a real stdio MCP server first, then implement.

## Depends on
McpServer trait + ToolMetadata.

## Implementation Notes
- `CliServer` lives in `crates/swissarmyhammer-plugin/src/server/cli.rs`, wired via `mod cli;` in `server.rs` and re-exported from `lib.rs`.
- Built on the rmcp client SDK: `rmcp::transport::TokioChildProcess::new` spawns the child; `rmcp::service::serve_client` performs the `initialize` handshake; the resulting `Peer<RoleClient>` drives `list_all_tools` and `call_tool`. No JSON-RPC framing is hand-rolled.
- `notifications/tools/list_changed` is handled cleanly via a custom `rmcp::ClientHandler::on_tool_list_changed`, which re-lists tools and swaps the shared cache.
- `CallerId` is not sent on the wire (MCP `tools/call` has no caller field) — documented in the module docs.
- Kill-on-drop: holding the `RunningService` owns the child; its drop guard cancels the service and the rmcp child-process transport kills the child from its own `Drop`.
- Test fixture: a tiny rmcp stdio MCP server `[[bin]] cli_server_fixture` exposing an `echo` tool; the integration test locates it via `CARGO_BIN_EXE_cli_server_fixture`.
- Tests in `tests/cli_server.rs`: `invoke_round_trips_a_tools_call_over_the_subprocess`, `unknown_tool_yields_unknown_tool_error`, `dropping_the_server_terminates_the_child_process` (asserts PID gone via `kill(2)` signal 0). All awaits use bounded timeouts.

## Review Findings (2026-05-16 18:30)

### Warnings
- [x] `crates/swissarmyhammer-plugin/tests/cli_server.rs` — No test exercises the crash-handling path the task scrutiny list and the module docs (`cli.rs:34-38`) explicitly promise: "a crashed subprocess... every subsequent `invoke` fails with `Error::ServerUnavailable`... no panic, no hang". The `unknown_tool_yields_unknown_tool_error` test does not cover it — `invoke`'s local cache guard (`cli.rs:253`) short-circuits with `UnknownTool` before any wire call, so that test never reaches `map_service_error` or a closed transport. Add an integration test that connects the fixture, kills the child (or makes it exit), then asserts a subsequent `invoke` of a *known* tool returns `Error::ServerUnavailable` within a bounded timeout. This is the one acceptance-relevant behavior with zero coverage.

### Nits
- [x] `crates/swissarmyhammer-plugin/src/server/cli.rs:271` — `CliServer` is split across two `impl` blocks (`connect`/`child_pid` at line 143, `tools_snapshot` at line 271) straddling the `impl McpServer` block. `tools_snapshot` is a private helper used by the trait impl; fold it into the first `impl CliServer` block (or place it adjacent) so the inherent methods are not visually fragmented. Purely organizational — no behavior change.
- [x] `crates/swissarmyhammer-plugin/src/server/cli.rs:257` vs `src/server/in_process.rs:217` — The two transports build the same `CallToolRequestParams` differently: `cli.rs` passes a bare `String`, `in_process.rs` wraps it in `Cow::Owned`. Both compile (`String: Into<Cow<'static, str>>`); the bare-`String` form in `cli.rs` is the cleaner one. Consider aligning `in_process.rs` to match, so the sibling transports read consistently. Out of this task's strict scope — flag only.