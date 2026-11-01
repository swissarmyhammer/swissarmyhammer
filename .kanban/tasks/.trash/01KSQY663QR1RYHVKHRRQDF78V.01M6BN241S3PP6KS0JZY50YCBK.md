---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffbf80
project: llama-coverage
title: Make AgentServer::generate tool-loop deterministically testable (parallel dispatch + retry loop)
---
## DONE (2026-05-28)

Covered both target paths deterministically — **without** the production refactor option 1 proposed. During implementation I found a lower-risk seam: the tool-dispatch methods (`execute_tools_parallel`, `execute_tool_with_retry`, `execute_tool`, `check_tool_capability`, `validate_tool_arguments`) touch only the MCP client / dependency analyzer / client capabilities — never the model — and `ModelManager::new` defers loading (the queue worker just idles on its channel when no generation request is submitted). So a **headless `AgentServer`** can be constructed in-test and its real methods driven directly against a scripted `MCPClient`. No `ToolExecutor` extraction, no change to the production tool path, no flakiness.

### What landed (all in `agent.rs` `#[cfg(test)] mod tests::tool_execution`)
- `ScriptedMcpClient` — an `MCPClient` whose `call_tool` outcome is a closure of `(call_index, name)`, recording every call so attempt counts are assertable.
- `headless_agent(mcp_client)` — builds an `AgentServer` whose model is never loaded.
- 4 deterministic tests:
  - `execute_tools_parallel_runs_every_call_and_preserves_ids` — 2 calls, both succeed, `call_id`s line up with inputs, both invoked. Covers the `join_all` parallel path.
  - `execute_tools_parallel_captures_a_failing_tool_without_dropping_others` — one tool 400-fails; its error surfaces as a `ToolResult` error while the other succeeds. Covers the per-call error branch.
  - `execute_tool_with_retry_recovers_after_a_retriable_failure` — first attempt `503` (retriable) → retry succeeds; asserts ≥2 invocations and the recovered result. Covers the retry loop.
  - `execute_tool_with_retry_fails_fast_on_non_retriable_error` — `404` → exactly ONE attempt, surfaced as error. Covers the fail-fast path.

### Coverage
- `agent.rs`: 57.89% → **65.09%** (690/1060). Crate total **85.19%** (was 85.04%).
- Full suite green: 945 lib + 98 real-model + 225 + 19, 0 failures. Coverage gate still PASSES.

### Acceptance criteria
- [x] `execute_tools_parallel` covered by a deterministic test (no model flakiness).
- [x] `execute_tool_with_retry` fail→retry→succeed loop covered deterministically (plus the fail-fast path).
- [x] No flaky tests introduced; no real model used at all (headless AgentServer + scripted MCP client).

### Note for the gate card
The gate floor (80%) still has headroom; not raising it here to keep margin for model-availability variance, but `agent.rs` is now well above its prior level.