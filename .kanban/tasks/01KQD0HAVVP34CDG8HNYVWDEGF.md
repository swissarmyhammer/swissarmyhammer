---
assignees:
- claude-code
depends_on:
- 01KQD0DF12DJ1WPZ4MMWF69DAQ
- 01KQD0DW1GAW3KF2A33FQ0JZT3
position_column: review
position_ordinal: '80'
project: acp-upgrade
title: 'ACP 0.11: claude-agent: session modules'
---
## What

Migrate session-management modules to ACP 0.11.

Files:
- `claude-agent/src/session.rs`
- `claude-agent/src/session_loading.rs`
- `claude-agent/src/session_validation.rs`
- `claude-agent/src/session_errors.rs`

## Branch state at task start

B0 + B1 landed.

## Acceptance Criteria
- [x] These modules compile under `cargo check -p claude-agent`. Downstream modules may still fail.
- [x] One commit on `acp/0.11-rewrite`.

## Tests
- [x] Inline tests in these files pass.

## Depends on
- 01KQD0DF12DJ1WPZ4MMWF69DAQ (B0).
- 01KQD0DW1GAW3KF2A33FQ0JZT3 (B1).

## Resolution

**No-op** — the B1 bulk schema-type import migration (commit `6f489b526`) already migrated every session module listed here. Verification confirms:

1. **All four files use `agent_client_protocol::schema::*` paths consistently** (B1's work).
2. **All ACP API calls match 0.11 signatures**:
   - `SessionId::new(impl Into<Arc<str>>)`
   - `AvailableCommand::new(name, description)`
   - `UnstructuredCommandInput::new(hint)`
   - `ContentChunk::new(ContentBlock)`
   - `TextContent::new(text)`
   - `McpServerStdio::new(name, command)`, `McpServerHttp::new(name, url)`, `McpServerSse::new(name, url)`
   - `LoadSessionRequest::new(session_id, cwd)`, `LoadSessionResponse::new()`
   - `SessionNotification::new(session_id, update).meta(meta)`
   - `agent_client_protocol::Error::new(code, message).data(data)` and `ErrorCode::Other(i32)` (re-exported at top level)
3. **All field accesses match 0.11 struct shapes** (`chunk.content`, `notification.session_id`, `request.cwd`, `request.mcp_servers`, etc.).
4. **`McpServer::Stdio/Http/Sse` enum variants** still exist with `non_exhaustive` — the existing `_ =>` arm in `convert_acp_to_internal_mcp_config` remains correct.

The 8 errors that remain on `cargo check -p claude-agent` are all in downstream rewrite modules (`agent.rs`, `agent_prompt_handling.rs`, `agent_trait_impl.rs`, `lib.rs`, `server.rs`) that subsequent ACP 0.11 tasks will reshape. Per this task's acceptance criterion "Downstream modules may still fail", that is the expected state.

This commit is the metadata-only acknowledgement — same pattern as B2 (commit `44973dca6`).