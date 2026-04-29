---
assignees:
- claude-code
depends_on:
- 01KQ367HE0Z8ZSXY90CTT8QYGG
position_column: todo
position_ordinal: ff8180
project: acp-upgrade
title: 'ACP 0.11: claude-agent: bulk schema-type import migration'
---
## What

Mechanical pass over `claude-agent/src/` (and `tests/common/`, `tests/integration/`) to migrate schema-type imports from `agent_client_protocol::X` → `agent_client_protocol::schema::X` everywhere those types still exist in 0.11.

Note: `Agent` itself is **not** moved to schema in this task — that's part of the per-module API reshape tasks. This task only moves the *schema* types: `SessionUpdate`, `ContentBlock`, `ToolCall`, `ToolCallStatus`, `ToolKind`, `StopReason`, `SessionId`, `SessionNotification`, `InitializeRequest`/`Response`, `NewSessionRequest`/`Response`, `LoadSessionRequest`/`Response`, `SetSessionModeRequest`/`Response`, `PromptRequest`/`Response`, `AuthenticateRequest`/`Response`, `CancelNotification`, `ExtRequest`/`Response`/`Notification`, `ClientCapabilities`, `FileSystemCapabilities`, `ImageContent`, `TextContent`, `ContentChunk`, `RawValue`, `Plan`, `PlanEntry`, `PlanEntryStatus`, `PlanEntryPriority`, `McpServer*`, `ProtocolVersion`, etc.

Use `cargo check -p claude-agent --all-targets` after the rename pass to find any leftover stale paths. Many compile errors will remain — those belong to subsequent tasks (B2 onwards).

## Branch state at task start

`acp/0.11-rewrite` with commit `d5b5465bd` and B0 housekeeping landed (or in parallel — order doesn't matter for these two).

## Acceptance Criteria
- [ ] No `use agent_client_protocol::X` (where X is a schema type) remains in `claude-agent/src/`, `tests/common/`, `tests/integration/`.
- [ ] After the bulk rename, `cargo check -p claude-agent --all-targets` produces *fewer* errors than before. (It will not pass yet — Agent trait reshape is in later tasks.)
- [ ] One commit on `acp/0.11-rewrite`.

## Tests
- [ ] No new tests; this task does not change behavior.

## Workflow
- Sed-style mechanical rename. Don't touch API usage shape.
- If a type *moved or was renamed* in 0.11 (not just relocated to schema), document it in the task comments and apply the rename here.

## Depends on
- 01KQ367HE0Z8ZSXY90CTT8QYGG (spike).