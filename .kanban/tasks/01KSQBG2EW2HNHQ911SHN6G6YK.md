---
assignees:
- claude-code
depends_on:
- 01KSQBCTMV4K3ATFZ5RFQ0FJBB
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffb680
project: llama-coverage
title: Cover ACP message translation (acp/translation.rs) — both directions, pure logic
---
## What

`crates/llama-agent/src/acp/translation.rs` (3k lines) maps between ACP protocol messages and llama-agent's internal types. It is pure data transformation and sits directly on the path between the kanban webview and the agent — a translation bug means the UI shows nothing or shows garbage even when generation is fine. Cover both directions.

## Cover

- **ACP → internal** — `session/prompt` request → internal generation request: content blocks, roles, tool definitions, session id handling.
- **Internal → ACP** — generation chunks/results → ACP `session/update` notifications and the final response: text deltas, tool-call requests, stop/finish reasons, usage/token counts.
- **Content block variants** — text, tool-call, tool-result, image/resource if supported; each round-trips.
- **Error mapping** — internal errors → ACP error responses with correct codes (the `-32603` seen in the bug log; the typed follower/queue errors).
- **Round-trip property** — for representative messages, ACP → internal → ACP preserves semantics.

## Acceptance Criteria

- [x] Both translation directions covered for every content-block variant the code handles.
- [x] Error → ACP-error-code mapping pinned.
- [x] `acp/translation.rs` region coverage reaches the epic threshold (target >95%).
- [x] No real model and no live transport — pure translation unit tests.

## Tests

- [x] Unit tests in `acp/translation.rs` `#[cfg(test)]` or `acp/translation/tests.rs`.
- [x] Run: `cargo test -p llama-agent translation` and confirm the coverage delta.

## Workflow

- Use `/tdd`. Pure logic — independent of the scripted-model harness.

## Implementation Notes

Added 30 tests (98 → 128 passing) covering the previously-untested arms, all inline `#[cfg(test)]` in `translation.rs` (no shared-file edits):

- ACP → internal content blocks: `ResourceLink` (rendered to text; the previously-commented-out gap), `Image`/`Audio` (UnsupportedContent), early-stop on first unsupported block.
- Error → JSON-RPC code mapping, including the `-32603` internal-error arms: `AgentError::Model`/`Queue` → -32603, `AgentError::MCP`/`Template` delegation; all `GenerationError` internal variants (Tokenization/Batch/Decoding/TokenConversion/Context/ContextLock/GenerationFailed) → -32603; `GenerationError::StreamClosed`; remaining `MCPError` (Connection/HttpTimeout/HttpConnection/Timeout); `TemplateError::Invalid`; `ValidationError` InvalidState/ContentValidation/SchemaValidation + Multiple nested codes; the `ToJsonRpcError for agent_client_protocol::Error` impl (internal_error -32603, invalid_params -32602, data forwarding, no-data).
- `ToolCallError` Display variants and `From<AgentError>`.
- Round-trip: ACP text → internal → ACP, multi-text ordering, stream-chunk text preservation.

Constructed typed content blocks directly (ImageContent::new / AudioContent::new / ResourceLink::new) instead of JSON to avoid the schema-version deserialization brittleness noted in the old commented-out test.

`cargo test -p llama-agent --lib acp::translation`: 128 passed, 0 failed. `cargo clippy -p llama-agent --lib --tests`: clean, no warnings.