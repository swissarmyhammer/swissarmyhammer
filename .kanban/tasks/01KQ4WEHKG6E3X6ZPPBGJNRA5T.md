---
assignees:
- wballard
position_column: done
position_ordinal: fffffffffffffffffffffff780
title: Fix shorted-out tool-call test path in llama-agent (real-model round-trip with non-empty tools)
---
**Problem:** llama-agent has no end-to-end test that exercises tool rendering and tool-call extraction against a real model. Every test session is constructed with `available_tools: Vec::new()`. The shared session-builder helpers hard-code empty tools. The Qwen3-0.6B test model loads, the docs claim "supports tool calling," but no real-model run ever sees a tool schema or has its output parsed for tool calls.

**Evidence (grep confirmed 2026-04-26):**

```
src/storage.rs:455              available_tools: Vec::new(),
src/session.rs:143              available_tools: Vec::new(),
src/queue.rs:1343               available_tools: Vec::new(),
src/agent.rs:1806               available_tools: Vec::new(),
src/tests/test_utils.rs         available_tools: Vec::new(),  // (4 occurrences)
src/validation/...              available_tools: vec![],      // (5 occurrences)
tests/coverage_tests.rs:34      available_tools: Vec::new(),
tests/integration/backward_compatibility.rs:37,265   available_tools: Vec::new(), vec![],
```

The only places `available_tools` is non-empty are unit tests *inside* `chat_template.rs` (lines 1442, 2000, 4549) — those test the template engine in isolation, not against a real model.

`extract_tool_calls` runs in `agent.rs:584` and `queue.rs:1044` on every generation, but on text the model produced without ever seeing tool schemas in its system message. Result is always "0 tool calls" — and no test ever notices, because no test expects otherwise. The test-coverage gap masks any breakage in the rendering or parsing paths for any strategy.

## What to change

### 1. Make the shared session helpers tools-aware

Update `llama-agent/src/tests/test_utils.rs` (and any equivalents in `centralized_test_utils.rs`) so the canonical "build a test session" helper accepts a `tools: Vec<ToolDefinition>` argument (or has a clearly named `*_with_tools` variant). All existing callsites that intentionally pass empty tools continue to do so explicitly; new tests can pass real tools without rolling their own session boilerplate.

### 2. Add a real-model tool-call integration test

In `llama-agent/tests/integration/`, add a new file (e.g. `tool_call_round_trip.rs`) that:

1. Loads the canonical test model (`TEST_MODEL_REPO = "unsloth/Qwen3-0.6B-GGUF"`, `TEST_MODEL_FILE = "Qwen3-0.6B-IQ4_NL.gguf"` from `src/test_models.rs`).
2. Constructs a session with at least one real `ToolDefinition` — pick something simple and unambiguous, e.g.:
   ```rust
   ToolDefinition {
       name: "read_file".to_string(),
       description: "Read a file from the filesystem".to_string(),
       parameters: json!({
           "type": "object",
           "properties": { "path": { "type": "string" } },
           "required": ["path"]
       }),
       server_name: "test".to_string(),
   }
   ```
3. Prompts the model to use it. Be explicit and direct: `"Use the read_file tool to read the file at /tmp/example.rs."` — single, unambiguous instruction.
4. Asserts:
   - `extract_tool_calls` on the model's response returns a non-empty `Vec<ToolCall>`.
   - The first ToolCall's `name == "read_file"`.
   - The arguments contain a `path` field whose value equals `/tmp/example.rs`.

This test is the *real* correctness signal. It exercises detection → input rendering → model generation → output parsing in one shot, against a real Qwen3 model.

### 3. Acknowledge it may fail today

This test, running on the **current** `Default` strategy with Qwen3-0.6B, is likely to either fail or extract zero tool calls — because the `Default` strategy doesn't target the wrapper tags Qwen3 actually emits per its tokenizer chat template. **That failure is the point.** The test fails meaningfully → we know the rendering and parsing need work → that work happens in `01KQ35KFJXJ70GNB4ZPRJD6R43`. Don't paper over the failure; if needed, mark the test `#[ignore]` with a TODO referencing the dependent task, but make sure the test exists and runs.

### 4. Don't expand scope into the strategy fix

This task **does not** add the new `Qwen3` strategy variant, fix detection, or change the parser. Those are the dependent task's job. This task adds the test infrastructure that makes the dependent task's correctness verifiable.

## Acceptance

- A real-model tool-call integration test exists in `llama-agent/tests/integration/`.
- The shared session-builder helpers in `tests/test_utils.rs` (and any equivalents) accept tools without breaking existing callers.
- The test runs in `cargo test -p llama-agent` (potentially as `#[ignore]`-marked if it fails on `Default` strategy, with a comment pointing to `01KQ35KFJXJ70GNB4ZPRJD6R43`).
- `cargo test -p llama-agent` and `cargo clippy -p llama-agent --all-targets -- -D warnings` remain clean.
- Findings are documented: does Qwen3-0.6B emit *anything* tool-call-shaped under the current `Default` strategy, or is the rendering wrong from the start? Either answer is useful.

## Why this is its own task

Closing this gap unblocks `01KQ35KFJXJ70GNB4ZPRJD6R43` (Qwen3 strategy). It also benefits every other strategy retroactively — once the test path is real, future regressions in `OpenAI`, `Claude`, or `Qwen3Coder` strategies are catchable too. Splitting it out keeps the strategy task focused on the strategy, not on building test infrastructure.

## Sources

- `llama-agent/src/test_models.rs` — `TEST_MODEL_REPO`, `TEST_MODEL_FILE`, comment "supports tool calling which is required for MCP notification testing."
- `llama-agent/src/chat_template.rs::extract_tool_calls` (line 355) — the function the new test exercises.
- The 25-or-so call sites that hard-code `available_tools: Vec::new()` listed above. #llama-agent

## Review Findings (2026-04-26 13:35)

### Nits
- [x] `llama-agent/tests/integration/tool_call_round_trip.rs` (header doc comment showing the run command) — The example invocation `cargo test -p llama-agent --test agent_tests tool_call_round_trip -- --ignored --nocapture` includes `--ignored`, but the test is not marked `#[ignore]`. With `--ignored`, `cargo test` runs *only* ignored tests, so a reader following this command verbatim would silently get zero tests run for this name. Drop `--ignored` from the example so it reads `cargo test -p llama-agent --test agent_tests tool_call_round_trip -- --nocapture`.