---
assignees:
- claude-code
position_column: todo
position_ordinal: '9780'
project: ai-panel
title: 'AI panel: tool calls never leave ''pending'' — ToolCallUpdate is dropped'
---
## What

In the AI panel, a tool call renders a "pending" card that never advances to completed/failed. Root cause traced through the agent stream pipeline:

`crates/claude-agent/src/claude.rs` — `run_stream_loop` reads Claude CLI stream-json lines, runs each through `protocol_translator.stream_json_to_acp`, then `process_notification` dispatches the resulting `SessionUpdate`:

```rust
match notification.update {
    SessionUpdate::AgentMessageChunk(chunk) => Self::handle_message_chunk(ctx, chunk, state),
    SessionUpdate::ToolCall(tool_call)      => Self::handle_tool_call(ctx, tool_call, state),
    SessionUpdate::ToolCallUpdate(_) => {
        tracing::debug!("ToolCallUpdate notification forwarded");  // <-- it is NOT forwarded
        true
    }
    _ => true,
}
```

`protocol_translator` **does** correctly produce a `SessionUpdate::ToolCallUpdate` with `ToolCallStatus::Completed` / `Failed` (and result content) from the CLI's `tool_result` content items (`crates/claude-agent/src/protocol_translator.rs` — `try_handle_tool_result`, ~line 364-394). But `process_notification` **drops it**: the `ToolCallUpdate` arm logs a (false) "forwarded" message and returns `true` without doing anything.

`AgentMessageChunk` and `ToolCall` are converted into `MessageChunk`s and sent down `ctx.tx`; that `MessageChunk` stream is consumed by `crates/claude-agent/src/agent_prompt_handling.rs` (`process_stream_chunks`/`handle_streaming_chunk`), which emits the rich ACP notifications the webview receives. `ToolCallUpdate` never becomes a `MessageChunk`, so it never reaches `agent_prompt_handling.rs` and never reaches the webview.

Confirmed downstream-correct (do **not** change):
- `agent_prompt_handling.rs` `handle_streaming_tool_call` emits the initial `SessionUpdate::ToolCall` with `ToolCallStatus::Pending` — that is the "pending" card the user sees.
- The webview adapter is correct: `apps/kanban-app/ui/src/ai/conversation.ts` `applyToolUpdate` folds `tool_call_update` onto the matching `dynamic-tool` part by `toolCallId`, and the reducer routes `tool_call_update` → `applyToolUpdate`. It simply never receives the update.

The bug is solely that the tool-completion `ToolCallUpdate` is dropped in `claude.rs::process_notification` and never emitted to the webview.

## Approach

Route the tool completion through the same `MessageChunk` stream that already carries `ToolCall` and text to the webview (the proven live path; `process_stream_chunks` consumes it and also stores chunks for session history replay).

1. **`crates/claude-agent/src/claude.rs`** — in `process_notification`, handle `SessionUpdate::ToolCallUpdate`: convert it into a `MessageChunk` (use `ChunkType::ToolResult`) carrying the tool call id, the new `ToolCallStatus`, and the result content, and send it via `ctx.tx`. Extend `MessageChunk` / `ToolCallInfo` (or add a small `ToolResultInfo` struct) with the fields needed to carry id + status + output — they currently model only the call (`id`, `name`, `parameters`), not the result.
2. **`crates/claude-agent/src/agent_prompt_handling.rs`** — in `handle_streaming_chunk`, add a branch for the `ChunkType::ToolResult` chunk that emits `SessionUpdate::ToolCallUpdate` (with the carried status and output content) and stores it in the session message log for history replay — mirroring how `handle_streaming_tool_call` stores the initial `tool_call` (`session.add_message(...)`).

Correct the misleading `tracing::debug!("ToolCallUpdate notification forwarded")` text. There is no double-emission risk: the `MessageChunk` path currently emits zero `SessionUpdate::ToolCallUpdate`s, so this adds the only one.

## Acceptance Criteria
- [ ] A Claude CLI `tool_result` for a tool call results in a `SessionUpdate::ToolCallUpdate` (status `Completed`, or `Failed` for an error result) being emitted to the ACP client.
- [ ] The emitted `ToolCallUpdate` carries the same `toolCallId` as the initial `ToolCall` and includes the tool's result content/output.
- [ ] In the AI panel, a tool call card advances from pending to completed/failed once the tool finishes.
- [ ] The completion update is stored in the session message log so a reloaded/replayed session shows the tool as completed, not pending.
- [ ] No change to the webview `conversation.ts` adapter or to `handle_streaming_tool_call`'s initial pending emission.

## Tests
- [ ] In `crates/claude-agent/src/protocol_translator.rs` tests: confirm (or add) a test that a `user` message carrying a `tool_result` content item yields a `SessionUpdate::ToolCallUpdate` with `ToolCallStatus::Completed` and the result content; and that an error `tool_result` yields `Failed`.
- [ ] In `crates/claude-agent/src/claude.rs` `mod tests`: assert `process_notification` given a `SessionUpdate::ToolCallUpdate` sends a `MessageChunk` of `ChunkType::ToolResult` carrying the tool call id and status (previously it sent nothing).
- [ ] In `crates/claude-agent/src/agent_prompt_handling.rs` tests: feed a `ChunkType::ToolResult` `MessageChunk` through the streaming-chunk handler and assert a `SessionUpdate::ToolCallUpdate` notification is emitted with the right id/status.
- [ ] In `apps/kanban-app/ui/src/ai/conversation.test.tsx`: assert (or confirm existing coverage) that a `tool_call` followed by a `tool_call_update` with `status: "completed"` renders the tool part in the completed state.
- [ ] Run `cargo test -p claude-agent` and `cargo clippy -p claude-agent -- -D warnings` — all green.
- [ ] Run `cd apps/kanban-app/ui && npx vitest run src/ai/conversation.test.tsx` — green.

## Workflow
- Use `/tdd` — write the failing `process_notification` / chunk-handler tests first, then implement.
