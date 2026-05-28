---
assignees:
- claude-code
position_column: todo
position_ordinal: '8180'
project: ai-panel
title: 'ACP: don''t leak raw tool_call/think markup as agent text when a structured tool call is emitted'
---
## Why
In the kanban GUI the user saw the model's RAW text:
```
<think>

</think>

<tool_call>
{"name": "kanban", "arguments": {"op": "get board", "include_counts": true}}
</tool_call>
```
streamed as the assistant message AND, separately, the properly structured ACP `ToolCall`. The raw markup should NOT appear as visible agent text when it has been parsed into a structured tool call.

## Root cause
`crates/llama-agent/src/acp/server.rs` agentic turn loop (~lines 2056-2104): it streams EVERY generated chunk to the client as `AgentMessageChunk` (via `llama_chunk_to_acp_notification`), including `<think>…</think>` and `<tool_call>…</tool_call>` spans, and THEN parses the same `generated_text` with `extract_tool_calls` into structured `ToolCall` notifications. So the client receives both representations.

## What
Make the visible agent text exclude tool-call markup (and the empty/again-internal `<think>` reasoning blocks). Since generation streams token-by-token, implement a streaming-aware filter that tracks whether the cursor is inside a `<tool_call>…</tool_call>` or `<think>…</think>` span and suppresses those chunks from the `AgentMessageChunk` broadcast — while still accumulating the FULL text for `extract_tool_calls`. The structured `ToolCall` remains the only representation of the call the client sees.

## Acceptance Criteria
- [ ] When the model emits a tool call, the `AgentMessageChunk` text the client receives does NOT contain `<tool_call>`/`</tool_call>` (or its JSON) nor `<think>`/`</think>` spans.
- [ ] The structured `ToolCall` notification is still emitted and executed (tool_calls_executed >= 1).
- [ ] Plain (non-tool, non-think) assistant text is still streamed unchanged.
- [ ] A test drives a scripted/real turn whose output contains think + tool_call markup and asserts the streamed agent text is clean while the structured tool call is present.

## Notes
- The chat-template tool-call format is model-specific (`<tool_call>` for Qwen). Use the chat_template's known tool-call/think delimiters rather than hardcoding if a helper exists.