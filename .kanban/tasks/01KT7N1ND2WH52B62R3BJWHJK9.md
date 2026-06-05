---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffde80
title: 'Flaky: acp_multi_turn_dispatches_tool_and_threads_result — 0.6B model fails final-answer comprehension'
---
crates/llama-agent/tests/integration/acp_agentic_loop.rs:545 (acp_multi_turn_dispatches_tool_and_threads_result)

Symptom: the test fails its LAST assertion only — `the final text must reference 'main'`. All earlier hard guards PASS in every run: a ToolCall is broadcast, a Tool-role message is appended to the session, and the response meta reports tool_calls_executed >= 1. So the read_file tool IS dispatched and its result IS threaded back into the session.

The failure is the Qwen3-0.6B test model failing to read the threaded-back file content on the second turn. Its captured reasoning confabulates the tool result (one run: "the content is the string 'this is a test.'", then guesses the function is "multi_turn"/"example" from the path), never reading the real fixture (`fn main() { println!("hello"); }`). Across 4 bounded retries it never produces "main".

Not a product bug: the tool-result threading mechanism is proven correct by the sibling test `tool_call_round_trip_via_mcp::test_full_round_trip_with_mcp_fetched_tools_against_real_model`, which exercises the same MCP read_file -> real-model path and PASSES.

Not a branch regression: nothing under crates/llama-agent changed vs the merge-base (no diff, no uncommitted changes). The test docstring itself documents this path as non-deterministic real-model behavior with a bounded retry as a "safety net".

What was tried:
- Ran in isolation twice: fails both times at line 545 only (guards pass).
- Confirmed fixture exists with `fn main()`.
- Ran sibling MCP round-trip tests: all PASS, confirming threading is correct.

Possible fix: make the assertion model-robust (accept the model declining/echoing path content), or skip-on-low-capability like the rate-limit skip idiom already in the file, or use a stronger test model for this specific assertion. Do NOT weaken the hard tool-dispatch guards. #test-failure