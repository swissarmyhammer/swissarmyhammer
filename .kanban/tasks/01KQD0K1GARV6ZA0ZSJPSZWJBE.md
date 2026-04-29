---
assignees:
- claude-code
depends_on:
- 01KQD0EA8Q367A70AWY8MMZ79D
position_column: done
position_ordinal: ffffffffffffffffffffffff9580
project: acp-upgrade
title: 'ACP 0.11: llama-agent: raw_message_manager + mcp_client_factory + test_utils'
---
## What

Migrate the supporting acp modules to ACP 0.11.

Files:
- `llama-agent/src/acp/raw_message_manager.rs`
- `llama-agent/src/acp/mcp_client_factory.rs`
- `llama-agent/src/acp/test_utils.rs`

## Branch state at task start

C1 landed.

## Acceptance Criteria
- [x] These modules compile under `cargo check -p llama-agent`.
- [x] One commit on `acp/0.11-rewrite`.

## Tests
- [x] Inline tests pass.

## Verification

Verified via `cargo check -p llama-agent --lib --message-format=short`: zero
errors and zero warnings attributable to these three files. The remaining lib
errors all originate in `acp/server.rs` (`impl Agent for AcpServer`,
`AgentWithFixture` import) and are tracked by separate kanban tasks.

- `raw_message_manager.rs` has no `agent_client_protocol` references at all;
  it is pure `tokio::sync::mpsc` + file I/O. Its single inline `#[tokio::test]`
  exercises only the file writer and uses no ACP types.
- `mcp_client_factory.rs` already routes `agent_client_protocol::schema::McpServer`
  through C1's bulk pass and matches the three currently-known variants
  (`Stdio`, `Http`, `Sse`) plus a `_ =>` arm for `#[non_exhaustive]` future-proofing.
  The clarifying comment on the catch-all arm was expanded to document the
  `#[non_exhaustive]` rationale, mirroring the pattern from sibling C-task
  verification commits.
- `test_utils.rs` already references
  `agent_client_protocol::schema::SessionNotification` through C1's bulk pass
  and consumes `AcpServer::new` from `acp::server`.

## Depends on
- 01KQD0EA8Q367A70AWY8MMZ79D (C1).