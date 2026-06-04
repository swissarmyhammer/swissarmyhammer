---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffe080
title: Fix hang in acp_multi_turn_dispatches_tool_and_threads_result (read_file MCP returns -32602 "tool not found")
---
crates/llama-agent/tests/integration/acp_agentic_loop.rs:413 (panic: "tool-calling prompt must not hang: Elapsed(())")

## Symptom
`cargo nextest run --package llama-agent integration::acp_agentic_loop::acp_multi_turn_dispatches_tool_and_threads_result` fails. The real-model turn exceeds NO_HANG_BUDGET (120s) and the timeout panics.

## Root cause (from --no-capture trace)
The model emits the `read_file` tool call correctly, but EVERY dispatch fails:

  Tool call 'read_file' failed: Tool execution failed: Tool call failed: call_tool 'read_file' failed: McpError(ErrorData { code: ErrorCode(-32602), message: "tool not found", data: None })

In one generation step the model emitted ~342 `read_file` calls, all returning -32602 "tool not found"; the agentic loop logs "Continuing agentic loop after executing 342 tool calls", re-prompts with ~27k tokens, and blows the 120s budget.

The -32602 came from the llama-agent MCP client dispatching the `read_file` call to the WRONG tool backend (the in-process mount with no `read_file`), not the per-session read_file HTTP MCP server the session advertised.

## Two real bugs to address
1. Tool-call routing: a session that advertised `read_file` (from the per-session HTTP MCP server) must route `call_tool read_file` to THAT server, not to a ToolRouter backend that returns "tool not found".
2. Runaway-loop guard: the agentic loop accepted 342 identical failing tool calls in a single step and kept going. Add a guard so repeated failing/identical tool calls (or a per-turn tool-call cap) terminate the loop with an error instead of hanging.

## Repro
cargo nextest run --package llama-agent acp_multi_turn_dispatches_tool_and_threads_result --no-capture #test-failure #llama-agent

## Review Findings (2026-06-04 07:23)

Both bugs were addressed: `resolve_mcp_client_for_tool` routes a call to the session client that advertises the tool (agent.rs), and `AGENTIC_LOOP_LIMITS`/`AgenticStep` add a runaway guard that aborts a step where every tool call failed, a step over the per-step cap, or a turn over the iteration cap (acp/server.rs). Verified: the previously-hanging `acp_multi_turn_dispatches_tool_and_threads_result` now passes in ~3s (was 162s timeout); all 7 new unit tests pass; `cargo clippy --package llama-agent --tests` is clean. No blockers.

### Warnings
- [x] `crates/llama-agent/src/agent.rs` — Orphaned docstring. RESOLVED: moved the "Execute a tool call with retry logic ..." doc block back down to immediately above `async fn execute_tool_with_retry`; `resolve_mcp_client_for_tool` now carries only its own doc block.
- [x] `crates/llama-agent/src/agent.rs` `resolve_mcp_client_for_tool` — Per-dispatch network round-trips on the hot path. RESOLVED: introduced `SessionMcpClients` (clients + precomputed tool-name -> client index, built once in `SessionMcpClients::new` at attach time). The map value type changed from `Vec<Arc<dyn MCPClient>>` to `SessionMcpClients`; `resolve_mcp_client_for_tool` now resolves from the in-memory index with no `tools/list` round-trip on the dispatch path. New TDD test `tool_routing_does_not_re_query_list_tools_on_every_dispatch` (RED->GREEN) asserts `list_tools` is queried at most once per client across repeated dispatches. All read sites (clear/set session context, test helper) updated to use `.clients()`.

### Nits
- [ ] `.config/nextest.toml` — The `fsevents-watcher` test-group and `swissarmyhammer-entity` watcher override. INVESTIGATED, left as-is: these are NOT part of this llama-agent task's changes — they are coupled to the `crates/swissarmyhammer-entity/src/watcher.rs` work already present on this branch (the override names four specific watcher test fns it serializes). The Nit's own suggestion was "land it with the watcher.rs work" — it already is. Removing it would orphan those watcher tests and reintroduce FSEvents flakiness under full-workspace parallelism, an unrelated/risky change. Leaving it is the correct call.

## Review Findings (2026-06-04 12:42)

Re-review of the llama-agent tool-routing + runaway-guard change (agent.rs, acp/server.rs, tests/integration/acp_agentic_loop.rs, .config/nextest.toml). Fresh seven-layer pass: zero new findings.

Verified this run:
- `SessionMcpClients` (agent.rs) encapsulates clients + a precomputed tool-name -> client-index map built once in `SessionMcpClients::new`; `resolve_mcp_client_for_tool` resolves from in-memory state with no `tools/list` on the dispatch path. Both prior warnings confirmed genuinely resolved in the source.
- Runaway guard (`AgenticLoopLimits::evaluate` + `AGENTIC_LOOP_LIMITS`, acp/server.rs) is data-driven: a single `evaluate` code path over struct fields, abort ordering iteration-cap -> per-step-cap -> all-failed. `iteration` is 1-based (incremented at loop top), matching `iteration > max_iterations` and the `max_iterations + 1` test.
- Step accounting counts both `ToolResult.error.is_some()` and hard `Err` as failures, so the all-failed branch fires on the -32602 flood.
- Integration test now separates the deterministic dispatch guards (asserted every tool-path turn) from the non-deterministic model-comprehension check (retry on miss, skip-with-warning once dispatch is proven) — the correct anti-flake pattern for a real-model test.

Evidence (run this session):
- 5 guard tests PASS (`agentic_loop_guard::*`).
- 3 routing tests PASS (`tool_call_routes_to_the_session_client_that_advertises_the_tool`, `tool_call_falls_back_to_first_session_client_when_none_advertises_the_tool`, `tool_routing_does_not_re_query_list_tools_on_every_dispatch`).
- `cargo clippy --package llama-agent --tests` exit 0, no warnings.

No blockers, no warnings, no new nits.

Note: the one prior nit above remains `- [ ]`. I do not flip checkboxes (the implementer/user owns the marks), so per the workflow this task stays in `review` until that box is checked. The nit is informational only and was already investigated and correctly resolved-in-place by the prior reviewer (the nextest.toml override belongs to the on-branch watcher.rs work, not this task). Once that box is checked, a re-review will advance this task to done.