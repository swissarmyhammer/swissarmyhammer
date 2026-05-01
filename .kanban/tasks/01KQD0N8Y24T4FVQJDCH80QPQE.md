---
assignees:
- claude-code
depends_on:
- 01KQD0MMR7W64307S03XBV69BH
position_column: done
position_ordinal: ffffffffffffffffffffffffad80
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
- [x] `cargo check -p claude-agent --tests` passes.
- [x] One commit on `acp/0.11-rewrite`.

## Tests
- [x] `cargo nextest run -p claude-agent` — green (incl. previously-disabled inline tests now re-enabled by B0).

## Depends on
- 01KQD0MMR7W64307S03XBV69BH (B9).

## Implementation notes

The only ACP 0.11 break across the test surface was `claude-agent/tests/common/test_client.rs` — the file had `impl Client for TestClient` (Client is a unit Role marker in 0.11, not a trait). All other test files (handler_utils, content_blocks, fixtures, session_persistence, tool_call_permissions, coverage_tests, terminal_rate_limiting, user_approval_flow) compile against ACP 0.11 unchanged because they hit inherent methods on `ToolCallHandler`, `TerminalManager`, `SessionManager`, etc. — none of them call into the deleted `Client` / `Agent` traits.

Fix: drop the `#[async_trait(?Send)] impl Client for TestClient` block and lift the four method bodies (`read_text_file`, `write_text_file`, `session_notification`, `request_permission`) into a plain inherent `impl TestClient`. The internal `#[cfg(test)] mod tests` already calls them as inherent methods (`client.read_text_file(req).await`), so no test changes were needed. Module docs updated to explain the 0.11 shape — the inherent methods are the bodies a future test would put inside a `Client.builder().on_receive_request_from(Agent, ...)` callback when wiring a real `ConnectionTo<Client>` peer.

`cargo nextest run -p claude-agent`: 307/307 pass. `cargo clippy -p claude-agent --tests --no-deps`: zero warnings.