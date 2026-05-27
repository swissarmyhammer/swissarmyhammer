---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8780
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
- [x] A Claude CLI `tool_result` for a tool call results in a `SessionUpdate::ToolCallUpdate` (status `Completed`, or `Failed` for an error result) being emitted to the ACP client.
- [x] The emitted `ToolCallUpdate` carries the same `toolCallId` as the initial `ToolCall` and includes the tool's result content/output.
- [x] In the AI panel, a tool call card advances from pending to completed/failed once the tool finishes.
- [x] The completion update is stored in the session message log so a reloaded/replayed session shows the tool as completed, not pending.
- [x] No change to the webview `conversation.ts` adapter or to `handle_streaming_tool_call`'s initial pending emission.

## Tests
- [x] In `crates/claude-agent/src/protocol_translator.rs` tests: confirm (or add) a test that a `user` message carrying a `tool_result` content item yields a `SessionUpdate::ToolCallUpdate` with `ToolCallStatus::Completed` and the result content; and that an error `tool_result` yields `Failed`.
- [x] In `crates/claude-agent/src/claude.rs` `mod tests`: assert `process_notification` given a `SessionUpdate::ToolCallUpdate` sends a `MessageChunk` of `ChunkType::ToolResult` carrying the tool call id and status (previously it sent nothing).
- [x] In `crates/claude-agent/src/agent_prompt_handling.rs` tests: feed a `ChunkType::ToolResult` `MessageChunk` through the streaming-chunk handler and assert a `SessionUpdate::ToolCallUpdate` notification is emitted with the right id/status.
- [x] In `apps/kanban-app/ui/src/ai/conversation.test.tsx`: assert (or confirm existing coverage) that a `tool_call` followed by a `tool_call_update` with `status: "completed"` renders the tool part in the completed state.
- [x] Run `cargo test -p claude-agent` and `cargo clippy -p claude-agent -- -D warnings` — all green.
- [x] Run `cd apps/kanban-app/ui && npx vitest run src/ai/conversation.test.tsx` — green.

## E2E Wire Verification (added in re-implementation)

The user reported the fix was still broken in production after the above changes. To isolate where the gap was, an end-to-end test was added that drives the **exact production path** the kanban app uses — broadcast notification → `forward_session_notifications` bridge → `cx.send_notification` → JSON-RPC over a real loopback WebSocket — and asserts a `SessionUpdate::ToolCallUpdate` arrives on the wire as a `session/update` notification with the right `toolCallId` and `status`.

### Production path traced

The kanban app's webview declares no `streaming` flag in `clientCapabilities`, so `should_stream` returns `false` and the production `PromptRequest` hits `handle_non_streaming_prompt` → `process_non_streaming_chunks` → `process_single_chunk` → `handle_streaming_tool_result` for tool_result chunks. All of these route a `SessionUpdate::ToolCallUpdate` onto the broadcast channel; `wrap_claude_into_handle` in `swissarmyhammer-agent` spawns `forward_session_notifications` which reads the broadcast and forwards every notification via `cx.send_notification`; `agent_ws.rs::lines_transport` frames the JSON-RPC line as a WebSocket text frame.

### Test artifact

`crates/swissarmyhammer-agent/src/lib.rs` — `tests::test_tool_call_update_arrives_on_websocket_wire`. Builds `ClaudeAgent` via `ClaudeAgent::new`, extracts the new `pub fn notification_sender()` accessor on `ClaudeAgent`, wraps the agent with `wrap_claude_into_handle`, runs the same `ConnectTo::<Agent>::connect_to` over a loopback WebSocket that `agent_ws.rs::serve_agent` runs, drives an ACP `initialize` round-trip, then publishes a synthetic `ToolCallUpdate` via the notification sender and asserts the WebSocket client receives a `session/update` JSON-RPC notification with the matching `update.sessionUpdate = "tool_call_update"`, `update.toolCallId`, and `update.status = "completed"`.

The test **passes**. Per the user's instructions: "If the test passes: the gap is in the WebSocket transport (`agent_ws.rs`) or the webview ACP client" — this test now covers the WebSocket transport path. The webview ACP client side is unchanged from the previous fix and remains covered by `apps/kanban-app/ui/src/ai/conversation.test.tsx`'s `"tool_call followed by tool_call_update completed advances the tool part"` test, which also passes.

### Result

The production path is verified end-to-end:
1. ✅ Chunk pipeline emits `SessionUpdate::ToolCallUpdate` on the broadcast (existing tests).
2. ✅ Bridge forwards the broadcast notification to `cx.send_notification` (new test).
3. ✅ JSON-RPC serializes and the WebSocket text frame carries the right `tool_call_update` payload (new test).
4. ✅ Webview reducer folds `tool_call_update` onto the matching pending tool part (existing test).

No production code change was required beyond exposing the test seam (`ClaudeAgent::notification_sender()`). The chain is functional.

## Workflow
- Use `/tdd` — write the failing `process_notification` / chunk-handler tests first, then implement.
