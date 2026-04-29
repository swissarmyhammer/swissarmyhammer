---
assignees:
- wballard
position_column: done
position_ordinal: ffffffffffffffffffffffff8880
title: Verify multi-turn tool-use loop within a single prompt() call (tool result → continued generation → verdict)
---
**Untested critical path inside `prompt()`.** When qwen calls a tool inside a validator session, the expected flow within a single `agent.prompt(...)` call is:

1. Model emits `<tool_call>{"name": "read_file", ...}</tool_call>`.
2. llama-agent parses the tool call (Qwen3 strategy).
3. llama-agent dispatches the call through the MCP client to the validator MCP server.
4. Tool result comes back.
5. llama-agent appends the tool result to the conversation as a tool-role message (per the Qwen3 chat template's expected format).
6. llama-agent invokes the model again to continue generation with the tool result in context.
7. Model emits more text — possibly more tool calls, eventually the final `{"status": ..., "message": ...}` verdict.
8. `prompt()` returns to the caller with `stop_reason: EndTurn` and the full final response collected from notifications.

The Qwen3 strategy round-trip test (`tool_call_round_trip.rs`) only covers step 1-3 (model emits tool call, parser extracts it). Steps 4-7 — **the actual agentic loop** — are unverified. If any of them is broken, qwen will emit one tool call, get nothing back, and `prompt()` will return whatever junk text follows the tool call attempt instead of a real verdict.

## What to verify

### 1. Read llama-agent's existing agentic-loop implementation

Find where `prompt()` runs the inner loop in llama-agent. Likely in `llama-agent/src/agent.rs` or `llama-agent/src/queue.rs` near `extract_tool_calls`. Confirm:
- After parsing tool calls (line ~584 in `agent.rs`), are the tools actually dispatched, or is parsing the loop's terminal step?
- If dispatched, where does the result land in the conversation history before re-invoking the model?
- Does the loop respect Qwen3's expected `<|im_start|>tool` role-message format for tool results?

If any of these is missing or broken, that's the gap to fix.

### 2. Add a multi-turn integration test against Qwen3-0.6B

Extend `tool_call_round_trip.rs` (or new file `tool_use_multi_turn.rs`):

1. Construct a session with `read_file` tool registered.
2. Use a deterministic test fixture: a small text file with known content (e.g. `tests/fixtures/example.rs` with `fn main() { println!("hello"); }`).
3. Prompt the model: `"Use the read_file tool to read tests/fixtures/example.rs, then tell me what function it defines."`
4. Single `agent.generate(...)` call.
5. Assert in order:
   - The model's response references `main` or `fn main` (proves the tool result reached the model).
   - Final `stop_reason` is `EndTurn`.
   - Recording shows: `tool_call` notification → `tool_result` notification → continued `agent_message_chunk` notifications.

The crucial assertion is the last one: the response references content from the tool result. If the tool was never dispatched, qwen has no way to know the file's content and the assertion fails.

### 3. Add a validator-shaped multi-turn test

The validator pattern is: read file → judge → emit JSON. Mirror that in a test:

1. Same setup as above, but the prompt is the actual rule template format.
2. Add a fake rule body that says: `"Read the file mentioned in the changed_files list. If it contains the function main, return {\"status\": \"failed\", \"message\": \"main is forbidden\"}. Otherwise return {\"status\": \"passed\", \"message\": \"ok\"}."`.
3. `changed_files: ["tests/fixtures/example.rs"]`.
4. Run the full validator pipeline against this rule.
5. Assert the parsed verdict is `{"status": "failed", "message": "..."}` with the right name → proves the model read the file via the tool, applied the rule, and emitted the verdict in one `prompt()`.

This test sits at the validator layer, exercising both llama-agent's loop *and* the runner's verdict parsing in one shot.

## What to fix if the loop is broken

If the existing llama-agent code stops after parsing tool calls and never dispatches them or feeds results back, this card pivots from verification to implementation:

- Wire `extract_tool_calls` results into actual MCP dispatch via the agent's MCP client.
- Append the result to the conversation as a `tool` role message.
- Re-invoke the model in the same `prompt()` call.
- Loop until `stop_reason: EndTurn` *with no remaining unhandled tool calls*.

## Acceptance

- Multi-turn integration test passes: model reads a file via the tool, references its content in the response, stops cleanly.
- Validator-shaped multi-turn test passes: model emits a JSON verdict that depends on file content read via the tool.
- Recordings show the full `tool_call → tool_result → continued generation` pattern in their notification streams.
- `cargo test -p llama-agent` clean.

## Pairs with

- `01KQ7M0DRQ87EV8MD858QGMGJJ` (MCP fetch path verification). Different ends of the same chain: that one verifies tool *schemas* arrive; this one verifies tool *invocations* round-trip. Both must work.
- `01KQ35KFJXJ70GNB4ZPRJD6R43` (Qwen3 strategy, already done). That card proved parsing; this card proves the dispatch loop. #llama-agent

## Review Findings (2026-04-27 19:00)

### Nits
- [x] `llama-agent/tests/integration/read_file_mcp_server.rs:73-77` — `impl Default for ReadFileMcpServer` is dead code. The fixture is only constructed via `ReadFileMcpServer::new()` in `start_read_file_mcp_server`. Either drop the `Default` impl or `#[derive(Default)]` and remove `new()` to keep one canonical constructor.
- [x] `llama-agent/tests/integration/tool_use_multi_turn.rs:426` — Redundant OR in the verdict assertion: `lowered.contains("\"failed\"") || lowered.contains("\"status\": \"failed\"")`. The right arm is a stricter superstring of the left, so the right arm being true always implies the left arm is true. Drop the right arm or replace with a single `contains("\"failed\"")` for clarity.