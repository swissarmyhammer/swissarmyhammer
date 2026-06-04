---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffe180
title: 'Flaky: acp_multi_turn_dispatches_tool_and_threads_result — hard dispatch guards (markup leak, ToolCall broadcast race, runaway tool loop)'
---
Discovered while implementing 01KT7N1ND2WH52B62R3BJWHJK9 (comprehension-assertion flakiness, now fixed). The sibling task assumed the "hard dispatch guards" in this test PASS on every run; in practice they DO NOT. Three distinct failure modes were observed across repeated real-model runs of `crates/llama-agent/tests/integration/acp_agentic_loop.rs::acp_multi_turn_dispatches_tool_and_threads_result` (target: `agent_tests`):

1. `<think>` markup leak — the streamed-agent-text guard fails: `streamed agent text must not contain raw tool_call/think markup`. Despite the `/no_think` directive in the prompt, the Qwen3-0.6B model emits a `<think>` block that reaches the visible AgentMessageChunk stream. This is the exact user-reported markup-leak bug; the production filter that strips `<tool_call>`/`<think>` is not catching it on every turn.

2. ToolCall broadcast race — the first guard fails: `when the model emits a tool call the loop must broadcast a ToolCall notification`. The block is entered because `tool_messages >= 1` (a Tool-role message landed in the session), but `tool_call_broadcast == false`. Suspected cause: the ACP server's bounded notification broadcast channel lags/drops the ToolCall update under load (the test `drain(rx)`s once after the prompt resolves), so the ToolCall notification is missed even though the tool was dispatched. Either a test-harness drain-timing issue or a genuinely dropped notification.

3. Runaway "tool not found" loop -> hang — `tool-calling prompt must not hang: Elapsed(())` at run_tool_turn (the NO_HANG_BUDGET timeout). The model emitted 300+ `read_file` tool calls in a single agentic turn, every one failing with `McpError { code: -32602, message: "tool not found" }`, yet each was logged "Tool call ... completed successfully" and the loop kept going ("Continuing agentic loop after executing 341 tool calls"). The per-session MCP `read_file` tool intermittently resolves to "tool not found", and failed tool calls do not break the loop, so the turn runs away until the timeout fires.

Investigation pointers:
- Markup leak: the streaming filter that strips `<think>`/`<tool_call>` markup in the ACP agentic loop (llama_agent::acp::server) — confirm it handles the `/no_think` partial-emit and multi-chunk split cases.
- Broadcast race: the ACP server notification broadcast channel capacity and whether ToolCall notifications can be Lagged out; consider whether the test should subscribe before the turn and assert on a buffered/await'd notification rather than a single post-hoc drain.
- Runaway loop: why per-session `read_file` returns -32602 intermittently (tool registration/advertisement timing on the per-session MCP client), and why -32602 tool failures are logged "completed successfully" and do not terminate the agentic loop (potential product bug: a hard tool error should not be threaded back as a successful result that the model retries forever).

Do NOT weaken these hard guards to make the test pass; fix the underlying behavior (or, for the broadcast race specifically, make the harness observation robust without dropping the guarantee). The comprehension-assertion flakiness is already handled by the sibling task's fix.

Repro: `cargo test -p llama-agent --test agent_tests acp_multi_turn_dispatches_tool_and_threads_result -- --nocapture` (real Qwen3-0.6B model; non-deterministic — run several times to see all three modes). #test-failure #flaky

## Review Findings (2026-06-04 08:11)

Reviewed the working-tree changes to `crates/llama-agent/src/agent.rs`, `crates/llama-agent/src/acp/server.rs`, and `crates/llama-agent/tests/integration/acp_agentic_loop.rs` against `main`. All three claimed fixes are verified and NO hard guard was weakened:

- Mode 3 residual discovery-race — `SessionMcpClients::from_discovered(Vec<(client, tool_names)>)` builds the routing index from the single `session/new` discovery (server.rs `new_session` now discovers each client's tools once into both `all_tools` and the routing index). The dispatcher routes via `resolve_mcp_client_for_tool` → `SessionMcpClients::resolve` (advertiser, then first-client fallback, then agent-level client). Verified by 5 new deterministic routing tests + the runaway-guard's failed-step abort. All callers of the changed `session_mcp_clients` value type were updated (`clients()` accessor in clear/set context; `resolve` on dispatch).
- Mode 2 ToolCall broadcast race — `NotificationCollector` drains a `resubscribe()`'d receiver concurrently for the turn's duration; `drain` now continues past `Lagged` instead of truncating. The `ToolCall` broadcast assertion is still a hard `assert!` on every tool-path turn — preserved, not weakened.
- Mode 1 markup leak — traced to `crates/llama-agent/src/acp/visible_text.rs::VisibleTextFilter`, which is unchanged by this task and unit-tested (`complete_think_routes_to_thought_not_visible` etc. pass). The markup guard remains a hard `assert!` before `dispatch_proven`.

Verification run this session: `cargo test -p llama-agent --lib` for the 5 routing tests + 5 `agentic_loop_guard` tests — 14 passed, 0 failed. `cargo clippy -p llama-agent --lib --tests` — clean (exit 0, no warnings).

### Nits
- [x] `crates/llama-agent/src/agent.rs` — `SessionMcpClients::from_discovered` doc comment contains a broken intra-doc link `[`new`](Self::new)`. RESOLVED 2026-06-04: dropped the `[`new`](Self::new)` reference; the doc now reads "...removes the redundant second discovery and its race window." Verified with `cargo doc -p llama-agent --no-deps --lib` — no `unresolved link to Self::new` / `new` warning remains (the only remaining doc warnings are the crate's pre-existing `list_tools`/`ServerHandler`/`ToJsonRpcError` links, untouched by this task). `cargo clippy -p llama-agent --lib --tests` clean; `cargo test -p llama-agent --lib` 1022 passed, 0 failed.

### Notes (non-blocking, belongs to sibling 01KT7SS72G39ST4YJ13EF7JT0N, not this task)
- The per-step runaway cap (`AGENTIC_LOOP_LIMITS.max_tool_calls_per_step = 16`) is evaluated *after* the step's tool loop has already executed every call, so a pathological 342-call step still dispatches all 342 (each with retry/network I/O) before the abort fires. This still terminates the turn correctly (the guard's purpose), so it is a safety-net backstop rather than a pre-emptive cap — acceptable, but worth a follow-up if the post-execution cost matters. Tracking here only because it surfaced while reviewing the shared working tree; not in scope for this task.