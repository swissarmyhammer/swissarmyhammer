---
assignees:
- claude-code
position_column: todo
position_ordinal: '8380'
project: ai-panel
title: 'Bug: qwen produces 0 tokens on first prompt; retry hits \"Queue is full\"'
---
## What

When picking the `qwen` (Qwen3.6-27B GGUF) model and asking it any question in the AI panel, qwen returns an empty response. The GUI shows nothing. On retry the system shows \"AI Error\" because the previous request is still occupying the single-worker queue.

This is independent of bug `01KSNJ6AE18EQYDC2WSYFSSAY1` (the unknown-command issue) — it surfaces once qwen is actually wired up as the agent. May predate the ai-panel project entirely; needs investigation.

### Evidence

User reproduction: opened kanban, picked qwen, asked \"how many tasks do we have\". No answer appeared in the GUI. Retry produced an \"AI Error\" toast in the lower right.

OS log (`log show --predicate 'subsystem == \"com.swissarmyhammer.kanban\"'`):

```
15:24:23.089  Model loaded successfully in 18.613ms (Memory: +0 MB, Total: 0 MB)
15:24:23.102  Chat template engine initialized with strategy: Some(Qwen3) (derived from model: unsloth/Qwen3.6-27B-GGUF/Qwen3.6-27B-IQ4_NL.gguf)
15:24:24.120  [Llama] client→agent: request method=session/prompt
15:24:24.122  Processing prompt for session 01KSNHR5AQENQ2331VWCQJ3K87
15:26:26.782  Agent generation turn completed: 0 tokens in this turn, 0 total
15:26:26.796  No tool calls detected, ending agentic loop
15:26:26.796  Agentic loop completed: 0 tokens generated, 0 tool calls executed
15:26:26.818  [Llama] agent→client: response id=Some(Number(2))
15:29:06.926  F  Agent streaming generation failed: Request processing error: Queue is full
15:29:06.926  E  Sending error response  error=Error { code: -32603: Internal error, message: \"Request processing error: Queue is full\\n💡 Try reducing concurrent requests, increasing queue size, or adding more system resources\", data: None } id=Number(3)
```

### Two symptoms

1. **0 tokens generated** — the agentic loop completed with `0 tokens in this turn, 0 total` and `0 tool calls executed`. So qwen didn't even attempt to call the kanban MCP tool to count tasks. The model produced literally nothing.
2. **Queue jamming** — `worker_threads: 1, max_queue_size: 100`. After the empty response, the user's retry was rejected with \"Queue is full\". The first request appears to still occupy the single worker. May be a stuck future / un-finalized session in `llama-agent`.

### Suspect areas

- `crates/llama-agent/src/agent.rs` (or wherever agentic-loop lives): why does turn 1 generate 0 tokens? Empty prompt? Tokenizer mismatch? Stop-token hit immediately? Look for the prompt that's actually passed to llama.cpp and the raw decode output.
- Chat template: `Chat template engine initialized with strategy: Some(Qwen3)` — verify the Qwen3 template actually matches Qwen3.6-27B. The `Qwen3.6` model is one minor revision later; the template may not match and the model could be emitting an immediate end-token.
- Queue lifecycle: the worker doesn't release after a 0-token turn. Investigate `RequestQueue` cleanup on empty-response completion.

The model file resolved correctly:
```
Model downloaded to: /Users/wballard/.cache/huggingface/hub/models--unsloth--Qwen3.6-27B-GGUF/snapshots/.../Qwen3.6-27B-IQ4_NL.gguf
```

And `Model loaded successfully in 18.613ms (Memory: +0 MB, Total: 0 MB)` — the `0 MB` is suspicious. A 27B IQ4 weights load is ~14 GB. Either the log is wrong, or the load is lazy and weights aren't actually mapped. Worth investigating if this is the upstream cause of the empty generation.

### Files likely involved

- `crates/llama-agent/src/agent.rs`, `crates/llama-agent/src/queue.rs`, `crates/llama-agent/src/template.rs` (or equivalents).
- Chat templates / Qwen3 strategy definition (search for `Qwen3` in source).

## Acceptance Criteria

- [ ] Picking qwen and asking a simple question produces a non-empty response in the GUI.
- [ ] The agentic loop's token count is > 0 on a normal interaction.
- [ ] After a turn completes (success or empty), the worker is released and a subsequent prompt does not fail with \"Queue is full\".
- [ ] If the 0-token problem turns out to be a template / model mismatch, document the supported qwen variants and possibly drop the `kanban` tag from `qwen.yaml` until it works (and tag a known-working variant instead).

## Tests

- [ ] Add a Rust integration test in `crates/llama-agent/tests/` that drives a tiny qwen model (e.g. `qwen-0.6b-test.yaml` if it's usable) through a single prompt and asserts the response is non-empty and `tokens_generated > 0`.
- [ ] Add a queue-lifecycle test: after a turn returns (any outcome), a second prompt within the same session must enqueue successfully — no \"Queue is full\".
- [ ] Run: `cargo test -p llama-agent`.

## Reproduction

1. `make run` (or however the app is launched).
2. Pick `qwen` in the AI panel composer.
3. Wait ~7s for the model to load.
4. Ask: \"how many tasks do we have\".
5. Observe: empty response in chat; \"AI Error\" toast on retry.

## Related

- Found during manual testing after the ai-panel project's `/finish` run reported all four planned tasks done.
- Bug `01KSNJ6AE18EQYDC2WSYFSSAY1` is the per-board persistence regression — they are independent.