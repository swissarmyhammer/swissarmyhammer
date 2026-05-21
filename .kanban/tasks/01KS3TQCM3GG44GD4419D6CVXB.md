---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
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