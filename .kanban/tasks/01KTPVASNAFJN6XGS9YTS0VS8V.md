---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff9080
project: local-review
title: Restore agent-reply (AgentMessage) logging on the local review path
---
## Problem

On the calcutron run, **240 prompts** were issued to the local agent but the log contains **zero** `AgentMessage`/`AgentThoughtChunk` lines and zero agent-name-bracket tags (`[llama-…]`). There is no way, from the logs, to see what the model actually replied — you can only see that a prompt was sent (`session/prompt`) and whether the task failed or parsed. This is the gap that made it look like the review "might be working" when in fact it was dropping everything.

## Root cause (to confirm)

- `TracingAgent` (`agent-client-protocol-extras/src/tracing_agent.rs`) logs replies via its `trace_notifications` chunk-buffer (`AgentMessage (N chars): <full text>` at INFO).
- But the review driver collects notifications through its **own** path: `swissarmyhammer-validators/src/review/drive.rs` `build_pool_notifier` drains the agent's `notification_rx` broadcast straight into the pool's collectors — bypassing the `TracingAgent` notification logger.
- Additionally, `ChunkBuffer::flush` only fires when a *non-chunk* notification arrives; a pure-text turn (chunks then end-of-turn) may never flush. Verify whether an end-of-turn flush exists.

## Fix direction

- Emit a per-reply log line (full text, **untruncated** per the never-truncate rule — see [[feedback_never_truncate_logs]]) for the review's collected response, e.g. in `pool.rs` `run_prompt` after `collect_response_content`, or by ensuring the review path runs through `TracingAgent`'s notification logger.
- Note: `tracing_agent.rs:392-398` truncates `ToolCall` raw_input to 200 chars — review agents don't call tools so it doesn't bite here, but it violates the never-truncate rule; fix opportunistically.

## Acceptance criteria

- A `review … backend=local` run logs each fan-out/verify agent reply's full text (or a clear per-task response record), so a run is auditable end-to-end.
- The full reply text is not truncated.