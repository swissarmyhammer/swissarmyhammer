---
assignees:
- claude-code
position_column: todo
position_ordinal: f880
title: 'shell tests: McpServer::new() test call sites also create a `.shell` dir in the crate directory'
---
## What

While implementing ^0dystg9 (`ShellExecuteTool::new()` in `tool_config.rs` and `tests/integration/file_size_limits.rs` making a `.shell` dir), empirical verification found a much larger source of the same litter: `McpServer::new(...)` registers the real shell tool via `register_all_tools` -> `register_shell_tools` -> `ShellExecuteTool::new()`, which roots `ShellState` under the process CWD. `cargo nextest run -p swissarmyhammer-tools` runs each test binary with the crate directory as CWD, so every test that calls `McpServer::new(...)` also creates `crates/swissarmyhammer-tools/.shell`.

Reproduced directly: after fixing the 4 sites in ^0dystg9, `cargo nextest run -p swissarmyhammer-tools --lib` (all 1286 tests green) still recreates `.shell` in the crate directory. Running only `mcp::server::tests::` in isolation reproduces it on its own.

`new_with_work_dir(library, work_dir)` does **not** fix this either — `working_dir` only sets `ToolContext::working_dir` for path resolution; `register_all_tools` still calls `register_shell_tools` unconditionally, so `ShellExecuteTool::new()` still binds to the real process CWD regardless of which `McpServer` constructor is used.

## Call sites

All in `crates/swissarmyhammer-tools/src/mcp/server.rs`, `#[cfg(test)] mod tests`, pattern `let server = McpServer::new(TemplateLibrary::default()).await.unwrap();` (all already carry `#[serial_test::serial(cwd)]`, which only prevents races between CWD-touching tests — it does not isolate the shell state directory):

- test_validator_server_has_only_validator_tools
- test_validator_context_registry_is_isolated
- test_validator_server_serves_exactly_the_profile
- test_set_server_port
- test_set_server_port_updates_existing
- test_initialize_loads_prompts_into_library
- test_list_tools_returns_registered_tools
- test_execute_tool_unknown_tool_returns_error
- test_execute_tool_with_non_object_args
- test_execute_tool_has_tool_check
- test_reload_prompts_succeeds
- test_reload_prompts_detects_no_change
- test_stop_file_watching_is_safe_without_start
- test_stop_file_watching_returns_promptly_during_inflight_registration
- test_stop_file_watching_suppresses_late_store
- (one more inside a nested block near line 3325)
- test_get_tool_registry_shares_reference

Also `crates/swissarmyhammer-tools/src/mcp/tests.rs` — 3 sites, pattern `let server = McpServer::new(library).await.unwrap();` (lines ~63, ~114, ~167).

Found during the ^0dystg9 test pass (verified reproducible, not yet fixed — no `CurrentDirGuard` anywhere in this test, only a `work_dir` tempdir which does not isolate the shell state as established above):

- `crates/swissarmyhammer-mcp-proxy/tests/integration/end_to_end.rs::test_proxy_filters_tool_discovery` — calls `McpServer::new_with_work_dir(library, work_dir.path().to_path_buf())` with no `CurrentDirGuard`, then also starts a second real server via `start_mcp_server(McpServerMode::Http { .. })`, which goes through the same `register_all_tools` path. Pollutes `crates/swissarmyhammer-mcp-proxy/.shell`.

Checked and confirmed already safe (guarded with `CurrentDirGuard` before the `McpServer::new*` call, so no action needed):
`crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs`, `crates/swissarmyhammer-tools/tests/review_progress_stdio_test.rs`, `apps/swissarmyhammer-cli/src/mcp_integration.rs::test_cli_tool_registry_matches_server_registry`.

## Why this needs its own task

This is a different function (`McpServer::new`) in different files than ^0dystg9 named, with ~20 call sites instead of 4, and touches whether `McpServer` needs a test-only isolated-CWD constructor (mirroring `ShellExecuteTool::new_isolated()`) versus wrapping every call site in a `CurrentDirGuard`-pinned temp dir. That is a design decision, not a mechanical fix, so it is out of scope for ^0dystg9.

## Approach options (pick one during implementation)

1. Add a test-only `McpServer::new_isolated(...)` (or similar) that pins the shell state to a temp dir, mirroring `ShellExecuteTool::new_isolated()`, and switch all ~20+ test call sites to it (across `swissarmyhammer-tools` and `swissarmyhammer-mcp-proxy`).
2. Wrap each call site in a `CurrentDirGuard` pointed at a fresh `tempfile::TempDir`, consistent with the RAII test-isolation convention already used elsewhere in this file and in ^0dystg9's fix.

Option 1 is likely cleaner given the volume of call sites (one change point instead of ~20+, spanning two crates).

## Acceptance Criteria

- [ ] `cargo nextest run -p swissarmyhammer-tools --lib` leaves no `.shell` directory in `crates/swissarmyhammer-tools/`.
- [ ] `cargo nextest run -p swissarmyhammer-mcp-proxy` leaves no `.shell` directory in `crates/swissarmyhammer-mcp-proxy/`.
- [ ] No test in `server.rs`, `tests.rs`, or `mcp-proxy/tests/integration/end_to_end.rs` triggers `ShellExecuteTool::new()` while CWD is the crate directory.
- [ ] All existing tests in these files continue to pass.
#bug #shelltool #test-hygiene #tools