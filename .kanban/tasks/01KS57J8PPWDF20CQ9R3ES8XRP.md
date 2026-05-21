---
assignees:
- claude-code
position_column: todo
position_ordinal: '8580'
title: 'Fix flaky timeout: kanban-app state::tests open-board MCP tests (parallel contention)'
---
**File**: `apps/kanban-app` `state::tests` (binary `kanban-app`)

**Tests timing out** (observed 2026-05-21 during full-workspace `cargo nextest run --workspace`):
- `state::tests::test_open_board_serves_full_sah_mcp_toolset`
- `state::tests::test_open_second_board_keeps_both_in_list`

**Symptom**: Both time out at 300s under full-workspace nextest. Likely pass in isolation.

**Root cause hypothesis**: These open a board that stands up the full SAH MCP toolset (in-process MCP server); same parallel-contention family as the `mcp_integration.rs` timeouts — port assignment / HTTP handshake / client init starvation under ~13.7k-test parallel load.

**Reproducer**:
- `cargo nextest run --workspace` -> TIMEOUT [300s] for both
- Confirm: `cargo nextest run -p kanban-app -E 'test(test_open_board_serves_full_sah_mcp_toolset) | test(test_open_second_board_keeps_both_in_list)'`

**Suggested fix**: Investigate per-test isolation; follow the established `#[serial_test::serial(<group>)]` pattern to serialize the MCP-server-starting tests. Do NOT silence with --test-threads=1.

**Acceptance criteria**: full-workspace `cargo nextest run --workspace` completes with both tests passing.

**Pre-existing**: not caused by recent UI work itself but surfaces under contention. Discovered during implementation of task 01KS3TQCM3GG44GD4419D6CVXB.

#test-failure