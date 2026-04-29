---
assignees:
- claude-code
depends_on:
- 01KQD0MMR7W64307S03XBV69BH
position_column: todo
position_ordinal: ff9b80
project: acp-upgrade
title: 'ACP 0.11: claude-agent: integration tests + common helpers'
---
## What

Migrate claude-agent integration tests + common test helpers to ACP 0.11.

Files:
- `claude-agent/tests/common/test_client.rs`
- `claude-agent/tests/common/handler_utils.rs`
- `claude-agent/tests/common/content_blocks.rs`
- `claude-agent/tests/common/fixtures.rs`
- `claude-agent/tests/integration/session_persistence.rs`
- `claude-agent/tests/integration/tool_call_permissions.rs`
- `claude-agent/tests/integration/coverage_tests.rs`
- `claude-agent/tests/integration/terminal_rate_limiting.rs`
- `claude-agent/tests/integration/user_approval_flow.rs`

## Branch state at task start

B9 (agent.rs + lib.rs) landed.

## Acceptance Criteria
- [ ] `cargo check -p claude-agent --tests` passes.
- [ ] One commit on `acp/0.11-rewrite`.

## Tests
- [ ] `cargo nextest run -p claude-agent` — green (incl. previously-disabled inline tests now re-enabled by B0).

## Depends on
- 01KQD0MMR7W64307S03XBV69BH (B9).