---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffff180
title: 'Flaky real-model hook test: post_tool_use_additional_context aborts on 2nd-iteration failed tool call'
---
#test-failure

File: crates/llama-agent/tests/integration/acp_hooks_real_model.rs
Test: integration::acp_hooks_real_model::post_tool_use_additional_context_reaches_model_through_live_loop
Failing assertion: line 622 (`!outcome.loop_aborted_no_progress`)
Panic: "PostToolUse must not block — the successful tool call makes forward progress, so the loop must not abort"

FLAKE RATE: ~35% (7 failures / 20 isolated runs).

ROOT CAUSE (captured via RUST_LOG=llama_agent::acp::server=error):
On PASS runs the trace logs "Detected 1 tool calls" ONCE — the model emits read_file on iteration 1, it succeeds, turn ends.
On FAIL runs it logs "Detected 1 tool calls" TWICE. Iteration 1's read_file SUCCEEDS (so the hard guards pass: ToolCall broadcast + tool_message_count>=1, and the `hello`/marker content is actually present). Then on ITERATION 2 the small Qwen3-0.6B model emits a SECOND tool call that FAILS, so AGENTIC_LOOP_LIMITS.evaluate sees tool_calls=1, failed_tool_calls=1 and returns Abort("every one of the 1 tool call(s) in this step failed; the loop is not making progress") -> server.prompt() returns Err with data.agentic_loop_aborted -> loop_aborted_no_progress=true -> assert at 622 panics.

So the non-determinism is NOT the `hello` content (line 628) as originally reported — by the time line 622 fails, the read_file already ran successfully. The non-determinism is whether the 0.6B model emits a SECOND, malformed/failing tool call on the follow-up iteration after seeing the successful result.

Abort logic: crates/llama-agent/src/acp/server.rs AgenticLoopLimits::evaluate (the `failed_tool_calls == tool_calls` branch), invoked at the runaway guard in the agentic loop.

HARDENING DIRECTION (not applied — assessment only):
The test's invariant ("PostToolUse does not block, loop makes forward progress") is genuinely satisfied on iteration 1 every time. The flake is a downstream 2nd-iteration model artifact unrelated to the hook. Options to harden: (a) assert the hook effect on the FIRST tool turn's Tool message and treat a subsequent all-failed step as out-of-scope (don't gate on loop_aborted for the WHOLE turn); (b) constrain the prompt so the model stops after the first tool call; (c) treat agentic_loop_aborted-after-a-successful-tool-step as a non-fatal outcome for THIS assertion since the marker+hello are already present in tool_message_text. The PreToolUse sibling does NOT show this in isolation (11/12) but failed once under full-suite parallel load.