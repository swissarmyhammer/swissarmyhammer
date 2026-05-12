---
assignees:
- claude-code
depends_on:
- 01KQD0EA8Q367A70AWY8MMZ79D
position_column: done
position_ordinal: ffffffffffffffffffffffff9580
project: acp-upgrade
title: 'ACP 0.11: llama-agent: filesystem + terminal handlers'
---
## What

Migrate filesystem/terminal handler modules to ACP 0.11.

Files:
- `llama-agent/src/acp/filesystem.rs`
- `llama-agent/src/acp/terminal.rs`

## Branch state at task start

C1 landed.

## Acceptance Criteria
- [x] These modules compile under `cargo check -p llama-agent`.
- [x] One commit on `acp/0.11-rewrite`.

## Tests
- [x] Inline tests pass.

## Depends on
- 01KQD0EA8Q367A70AWY8MMZ79D (C1).

## Implementation notes

After the C1 bulk import migration (commit 9f4f564d0), these two modules
already use `agent_client_protocol::schema::*` for all schema types. ACP
0.11 keeps the field/builder shape used here:

- `filesystem.rs`: uses `ReadTextFileRequest::new`, `ReadTextFileResponse::new`,
  `WriteTextFileRequest::new`, `WriteTextFileResponse::new`. All four are
  `#[non_exhaustive]` in 0.11, and the builder-style `new(...)` constructors
  in the inline tests are the forward-compatible construction path.
- `terminal.rs`: only imports `ClientCapabilities` from the schema; the
  request/response newtypes for terminal lifecycle are local to this module
  and do not mirror the ACP schema's `terminal/*` types.

Verified via `cargo check -p llama-agent`: zero errors and zero warnings
attributable to `filesystem.rs` or `terminal.rs`. The remaining lib errors
all originate in `acp/server.rs` (`impl Agent for AcpServer` and
`AgentWithFixture` import) and are tracked by separate kanban tasks.

A clarifying module-level rustdoc was added to each file documenting the
ACP 0.11 type locations and `#[non_exhaustive]` shape for future readers.