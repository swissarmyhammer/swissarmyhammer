---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffc680
project: ai-panel
title: 'ACP: don''t leak raw tool_call/think markup as agent text when a structured tool call is emitted'
---
## DONE (2026-05-28)

The visible assistant message no longer leaks `<think>…</think>` reasoning or `<tool_call>…</tool_call>` markup; the structured ACP `ToolCall` remains the only representation of a call the client sees.

### Root cause (confirmed)
`acp/server.rs` turn loop broadcast EVERY generated chunk to the client as an `AgentMessageChunk` (raw text, including the control spans) and then parsed the same text into structured `ToolCall`s — so the client got both.

### Fix
- New `crates/llama-agent/src/acp/visible_text.rs`: `VisibleTextFilter`, a streaming state machine that strips `<think>…</think>` and `<tool_call>…</tool_call>` spans from the visible text while the FULL raw text still flows to `extract_tool_calls` and the response `_meta`. Handles tags split across chunk boundaries (`<tool` + `_call>`) and suppresses unterminated spans (model ran out of budget mid-`<think>`). 11 exhaustive pure unit tests, incl. the exact reported payload.
- `acp/server.rs` turn loop: feed each chunk's text through the filter and broadcast only the visible result (via new `translation::agent_message_notification`); flush held text at stream end. Raw `generated_text` (for tool extraction + meta) unchanged.
- Updated `acp_single_turn_streams_text_and_reports_tokens`: `/no_think` prompt; asserts streamed text is non-empty AND contains no markup, tokens>0, and the raw `llama_response` meta still contains the visible text (meta keeps full raw for debugging/titles).
- Added a direct regression assertion to `acp_multi_turn_dispatches_tool_and_threads_result`: on a real tool turn, the streamed agent text must contain no `<tool_call>`/`<think>` — the user's exact scenario.

### Verification
- 11 `visible_text` unit tests pass.
- Real-model `acp_single_turn` (clean stream, tokens>0) and `acp_multi_turn` (tool executed AND clean stream) pass.
- Full `acp_agentic_loop` module: 9 passed; translation lib tests: 128 passed. clippy clean.

### Acceptance criteria
- [x] Visible `AgentMessageChunk` text contains no `<tool_call>`/`</tool_call>` (or JSON) nor `<think>`/`</think>`.
- [x] Structured `ToolCall` still emitted + executed (tool_calls_executed >= 1).
- [x] Plain non-tool/non-think text still streamed unchanged (unit tests + single-turn).
- [x] A real turn with think + tool_call markup asserts clean stream + structured call present.

### Note (separate concern)
A turn that is ALL `<think>` with no answer (model exhausts budget mid-think — the slowness issue) now streams empty visible text. That is the thinking-budget problem, not this bug; the GUI could pass `/no_think` or use a non-thinking config to avoid it.