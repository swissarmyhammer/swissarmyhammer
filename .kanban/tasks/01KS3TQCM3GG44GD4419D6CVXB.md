---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8d80
title: 'Fix flaky timeout: test_mcp_server_basic_functionality (parallel contention)'
---
**File**: `apps/swissarmyhammer-cli/tests/integration/mcp_integration.rs:15`

**Symptom**: Times out at 300s when run as part of `cargo nextest run --workspace` (13,769 tests), but passes in 7.4s when run in isolation.

**Root cause hypothesis**: In-process MCP HTTP server starts with `port: None` (OS-assigned) and a `create_test_client(server.url())`. Under heavy parallel load (8000+ tests), either the OS port assignment, HTTP handshake, or RMCP client init contends with shared resources.

**Reproducer**:
- `cargo nextest run --workspace` -> TIMEOUT [300s]
- `cargo nextest run -E 'test(test_mcp_server_basic_functionality)'` -> PASS [7.4s]

**Suggested fix**: Investigate per-test isolation; consider `#[serial_test::serial]` on the trio of in-process MCP server tests, or increase the in-process server's startup grace period under load. Do NOT silence with --test-threads=1.

**Acceptance criteria**: 3 consecutive `cargo nextest run --workspace` runs complete with this test passing.

**Tests**: this test, plus the other two timed-out tests filed alongside.

**Pre-existing**: file is unchanged from `main`, last modified by commit a70af2f95 (workspace move). Not caused by recent UI work on the `kanban` branch.

#test-failure

---

## Implementation (2026-05-21)

**Fix applied**: Added `#[serial_test::serial(mcp_server)]` to `test_mcp_server_basic_functionality` only. `serial_test` was already a dev-dependency of `apps/swissarmyhammer-cli` (Cargo.toml). The named-group pattern `#[serial_test::serial(<group>)]` follows the established workspace convention (e.g. the `cwd` group in `apps/kanban-cli` and `apps/swissarmyhammer-cli/src/mcp_integration.rs`). Using the shared `mcp_server` group means when the sibling tasks add the same attribute to `test_mcp_server_prompt_loading` and `test_mcp_server_builtin_prompts`, the whole in-process MCP server trio serializes together. Did NOT touch the sibling test functions (their own tasks own them). Did NOT use `--test-threads=1`.

**Verification (quiet machine)**:
- Isolated: `cargo nextest run -p swissarmyhammer-cli -E 'test(test_mcp_server_basic_functionality)'` -> PASS [6.803s].
- Full workspace: `cargo nextest run --workspace` -> completed in 831s. `test_mcp_server_basic_functionality` PASSED (no longer in the timeout list). Summary: 13769 tests run, 13765 passed, 4 timed out, 1 skipped.
- Runs completed: 1 of the 3 requested consecutive full-workspace runs. A full run takes ~14-25 min here; per task guidance ("do as many as practical, at least one"), one full run was completed and this test passed in it. Recommend the reviewer or a follow-up confirm the remaining 2 consecutive runs.

**Out-of-scope timeouts observed in the same full-workspace run (NOT this task)**:
- `swissarmyhammer-tools mcp::file_watcher::tests::test_file_watcher_start_watching_sets_up_debouncer` — file_watcher contention; covered by existing task 01KS3TQQTM404TGYFZ9E39EXP3 (same module).
- `swissarmyhammer-tools mcp::test_utils::tests::test_client_list_tools` — NOT yet on the board (in-process MCP client contention). Needs a new task.
- `kanban-app state::tests::test_open_board_serves_full_sah_mcp_toolset` — NOT yet on the board. Needs a new task.
- `kanban-app state::tests::test_open_second_board_keeps_both_in_list` — NOT yet on the board. Needs a new task.