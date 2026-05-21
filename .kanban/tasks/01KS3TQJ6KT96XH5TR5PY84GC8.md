---
assignees:
- claude-code
position_column: todo
position_ordinal: '8180'
title: 'Fix flaky timeout: test_mcp_server_prompt_loading (parallel contention)'
---
**File**: `apps/swissarmyhammer-cli/tests/integration/mcp_integration.rs:43`

**Symptom**: Times out at 300s under full workspace nextest, but PASS [8.3s] in isolation.

**Root cause hypothesis**: Same as `test_mcp_server_basic_functionality` — in-process HTTP MCP server contention under heavy parallel load. Also uses `IsolatedTestEnvironment` which mutates `HOME` env var; possible cross-test interference even with the guard.

**Reproducer**:
- `cargo nextest run --workspace` -> TIMEOUT [300s]
- `cargo nextest run -E 'test(test_mcp_server_prompt_loading)'` -> PASS [8.3s]

**Suggested fix**: Mark with `#[serial_test::serial]` together with the other two failing in-process MCP server tests. Investigate whether HOME-env mutation in `IsolatedTestEnvironment` should serialize.

**Acceptance criteria**: 3 consecutive `cargo nextest run --workspace` runs complete with this test passing.

**Pre-existing**: not caused by recent UI work on the `kanban` branch.

#test-failure