---
assignees:
- claude-code
position_column: todo
position_ordinal: '9280'
project: llama-coverage
title: Make AgentServer::generate tool-loop deterministically testable (parallel dispatch + retry loop)
---
## Why

Follow-up from `01KSQVVYXHT7NJYPE85MBXJBS1` (agent.rs generate-path coverage). Two branches of `AgentServer::generate`'s tool loop are not covered because they cannot be driven deterministically today:

- `execute_tools_parallel` / `process_tool_calls` parallel path — needs the model to emit ≥2 tool calls in one turn.
- `execute_tool_with_retry`'s fail→retry→succeed loop — needs a tool that fails with a retriable error then succeeds.

`AgentServer` drives generation through `ModelManager::with_model` (a real model), NOT the `TextGenerator` trait, so the `ScriptedModel` test double cannot inject deterministic tool-call output into `generate()`. The tiny qwen-0.6B does not reliably emit structured/multiple tool calls on demand, so any test driving these via the real model would be flaky — which the real-path-tests rule says is worse than an honest gap.

## What to do (pick one)

1. **Generator-injection seam**: give `AgentServer` a test-only path to drive its tool loop from a scripted generator (mirror the `queue.rs` `#[cfg(test)] with_executor` / `QueueExecutor` pattern). Then write deterministic tests: a turn whose scripted output contains 2 independent tool calls (parallel), and a flaky in-process MCP tool that fails once with a `503`/`connection reset` then succeeds (retry loop). The retry-decision classifier `is_tool_error_retriable` is already exhaustively unit-tested.
2. **Flaky in-process MCP server alone** (lighter): add a `FlakyMcpServer` (fail-then-succeed) and a multi-tool prompt, accepting that it only covers the paths *when* the model cooperates — gate behind a retry/loop-until-cooperates or mark `#[ignore]`. Weaker; prefer option 1.

## Acceptance Criteria

- [ ] `execute_tools_parallel` covered by a deterministic test (no model flakiness).
- [ ] `execute_tool_with_retry` fail→retry→succeed loop covered deterministically.
- [ ] No flaky tests introduced; if a real model is used, output is not relied upon for control flow.

## Notes

- Coordinate with the coverage-gate threshold — landing this lifts `agent.rs` further and the gate floor can ratchet up.