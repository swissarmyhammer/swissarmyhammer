---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffd180
title: Make `<think>` work in the kanban AI panel (qwen reasoning needs to be visible, not eaten)
---
Qwen3.6's chat template emits a `<think>…</think>` reasoning span before the final reply. Reasoning is the point — we WANT the model to think before answering — but our pipeline currently makes it disappear.

## What's actually happening
Verified in the kanban log + reproduced in `mtp_smoke`:
- Turn 1: 31 tokens, tool call extracted, user saw something (the tool call surface).
- Turn 2 (post-tool-result): **100 tokens generated, 0 `AgentMessage` notifications delivered** to the client. The model spent its whole `max_tokens` budget inside an unclosed `<think>…` and our `VisibleTextFilter` swallowed every chunk waiting for the never-arriving `</think>`.

The filter is in `crates/llama-agent/src/acp/visible_text.rs` (or wherever it lives — `super::visible_text::VisibleTextFilter` from `acp/server.rs`'s turn loop). It buffers anything matching a `SUPPRESSED_SPANS` opener until the closer arrives; on stream end (`finish()`), buffered content inside an open suppression span is dropped — that drop is the bug.

## What "make think work" should mean
The user wants the model to think AND to see the result. Two genuine knobs we control:

1. **Budget**. `max_tokens` was 100 in this turn — Qwen's reasoning easily exceeds that for a tool-result digest. We compute `max_tokens` per-turn in `acp/server.rs::prompt` from "remaining context space". Confirm it really is letting the model use that, not getting clamped down somewhere. Even with enough room, an honest 100-token cap will still cut a thinking model off mid-thought.

2. **Surface think on truncation / on demand**. When generation ends inside an unclosed suppression span (`MaxTokens`, `EndOfSequence`, etc.), `VisibleTextFilter::finish()` should NOT drop the buffered span content — it should flush it, ideally tagged so the UI can render it as reasoning rather than a final answer. (Same shape would let a "show thinking" toggle work cleanly later.)

## Plan
- Audit the turn loop's `max_tokens` math in `acp/server.rs::prompt` and confirm a generous, honest budget (current `MAX_GENERATION_TOKENS = 16384` should already cover real thinks — verify nothing's clamping it to the 100 we observed).
- `VisibleTextFilter::finish()`: stop dropping buffered text when an open suppression span never closed. Either emit it verbatim, or — better — emit it through a distinct path the ACP server can route as a `Thought` chunk so the UI can render it distinctly. ACP has thought-content support; wire it.
- Test: drive a kanban-shaped prompt + tool-result continuation through `mtp_smoke` (extend the example if needed) and confirm a truncated-mid-think turn surfaces the model's reasoning instead of going silent.

Acceptance:
- The model can think out loud for a real number of tokens (≥ a couple hundred) without the budget cutting it off prematurely in normal use.
- If a turn DOES end inside an open `<think>`, the user sees the thinking content (in whatever shape the UI wants, but not nothing).
- A turn that closes its `<think>` and then continues with a reply behaves exactly as today — only the final reply is shown as the assistant message.

Files: `crates/llama-agent/src/acp/visible_text.rs`, `crates/llama-agent/src/acp/server.rs` (prompt loop max_tokens + chunk routing), tests/integration coverage for the truncated-think case.