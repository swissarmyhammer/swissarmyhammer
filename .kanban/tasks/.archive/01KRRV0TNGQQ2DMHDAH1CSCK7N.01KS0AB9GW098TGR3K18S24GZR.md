---
assignees:
- claude-code
position_column: todo
position_ordinal: '9080'
title: 'Unblock test_validator_shaped_multi_turn_with_real_model (currently #[ignore]d)'
---
File: crates/llama-agent/tests/integration/tool_use_multi_turn.rs (fn test_validator_shaped_multi_turn_with_real_model)

What: This test is marked #[ignore] and is the single "1 skipped" test reported by `cargo nextest run --workspace --all-features`. The test skill requires zero skipped tests.

Why it is skipped: The canonical CI test model `unsloth/Qwen3-0.6B-GGUF` (~600M params) is too small to reliably dispatch a tool call when the prompt is shaped as a conditional validator rule with a JSON verdict requirement. At temperature=0.0 the model reasons through the rule inside <think>...</think> and emits a verdict directly without ever calling the `read_file` tool. Two prompt-tightening attempts (imperative step-1 framing, `/no_think` directive) did not recover the call. The tool-dispatch path itself IS proven correct by two sibling tests that run green against the same model (test_tool_call_round_trip_with_real_model and test_multi_turn_tool_use_round_trip_with_real_model).

This is a genuine model-capability limitation, not a code regression — it cannot be fixed by editing test or production code. The docstring states the proper fix.

Acceptance Criteria:
- Wire a larger tool-capable model into crates/llama-agent/src/test_models.rs (or make the avp validator system prompt strict enough to survive Qwen3-0.6B-class models).
- Remove the #[ignore] attribute from test_validator_shaped_multi_turn_with_real_model.
- `cargo nextest run -p llama-agent --all-features` runs the test and it passes with 0 skipped.

Tests: cargo nextest run -p llama-agent --all-features (the test must run, not skip, and pass).

Tag: test-failure. Related historical context: kanban task 01KQG8P8M4FVH5JHJYNX2XBM6C. #test-failure