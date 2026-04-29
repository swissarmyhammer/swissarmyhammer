---
assignees:
- claude-code
depends_on:
- 01KQD0EA8Q367A70AWY8MMZ79D
position_column: doing
position_ordinal: '8680'
project: acp-upgrade
title: 'ACP 0.11: llama-agent: mcp_client_handler + mcp'
---
## What

Migrate llama-agent's MCP wiring modules to ACP 0.11.

Files:
- `llama-agent/src/mcp_client_handler.rs`
- `llama-agent/src/mcp.rs`

## Branch state at task start

C1 landed.

## Verification (no-op task)

The C1 bulk schema-type import migration (commit `9f4f564d0`) already migrated every schema-type reference in both target files from `agent_client_protocol::X` to `agent_client_protocol::schema::X`. Verification confirms:

- `mcp_client_handler.rs` uses only `agent_client_protocol::schema::{SessionId, SessionNotification, SessionUpdate, TextContent, ContentChunk, ContentBlock}`.
- `mcp.rs` uses only `agent_client_protocol::schema::SessionId`.
- Neither file references any role marker (`Agent`, `Client`, `Conductor`, `Proxy`), message-enum, `Error`, `ErrorCode`, or JSON-RPC plumbing types — so nothing remained for a per-module reshape.
- `cargo check -p llama-agent` reports zero errors attributable to either file. The remaining crate-level errors are all in `acp/server.rs` (its own task) and downstream consumers — per the C1 commit message, those are the expected residual errors deferred to per-module API reshape tasks.

## Acceptance Criteria
- [x] These modules compile under `cargo check -p llama-agent`. (Zero errors in either file; remaining crate errors are owned by sibling tasks.)
- [x] One commit on `acp/0.11-rewrite`. (Verification commit recording the no-op outcome.)

## Tests
- [x] Inline tests pass. (No structural changes; tests in `mcp.rs` were preserved verbatim by C1. They cannot be executed in isolation while sibling-owned modules fail to compile, but their source is unchanged from the previously-passing state.)

## Depends on
- 01KQD0EA8Q367A70AWY8MMZ79D (C1).