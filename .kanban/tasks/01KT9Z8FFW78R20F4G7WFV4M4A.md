---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffeb80
project: claude-hooks
title: Multi-turn agent path for LlamaHookEvaluator (tools + bounded loop)
---
Follow-up to 01KT9X9C6B7DXMN6PW84Q1S1CS. `LlamaHookEvaluator::evaluate` currently routes BOTH `prompt` (is_agent=false) and `agent` (is_agent=true) hooks through a single short model call (`AgentServer::generate_short`). The prompt path is fully live; the agent path is single-turn only.

## Scope
- Implement a true multi-turn agent loop for the `is_agent=true` branch: run the session's tools in a bounded turn loop (cap turns + tokens to avoid runaway), then have the model emit the final `{"ok": bool, "reason"?: string}` verdict JSON.
- Keep the existing `HookModel` generation seam testable without a GPU; add a scripted multi-turn fake driving the loop (mirror the `ScriptedModel` tool-call sequence pattern).
- Preserve the never-crash-the-turn contract: any loop/model error degrades to `{"ok": true}` (Allow).

## Notes
- The single-turn seam lives in `crates/llama-agent/src/acp/llama_hook_evaluator.rs` (`HookModel` trait + `LlamaHookEvaluator`). `generate_short` is in `crates/llama-agent/src/agent.rs`.
- The agentic tool loop already exists in `agent.rs` / `acp/server.rs`; reuse it rather than re-implementing tool dispatch.

## Review Findings (2026-06-04 14:22)

### Warnings
- [x] `crates/llama-agent/src/agent.rs` (`run_bounded_tool_loop`) — RESOLVED. Refactored `run_bounded_tool_loop` into a free async fn generic over a new `BoundedTurn` seam (two ops: `generate` and `process_and_append`). Production wires it through `SessionTurnDriver`, which calls the real `generate_once` / `process_tool_calls` / `append_tool_round`; the loop's control flow (turn counting, empty-vs-nonempty branch, between-turn append, forced final generation after the budget) is now exercised by unit tests `agent::tests::bounded_tool_loop::{stops_at_first_verdict_without_running_full_budget, forces_a_final_generation_when_budget_is_spent, respects_the_turn_cap}` driving the REAL loop against a scripted `BoundedTurn` driver. The integration test's `ScriptedAgentModel` no longer re-implements the loop — it is now a recording fake at the `generate_agent_eval` seam that asserts only path selection + verdict normalization.

### Nits
- [x] `crates/llama-agent/src/acp/llama_hook_evaluator.rs` — RESOLVED. With the real loop now unit-tested, `HOOK_AGENT_MAX_TURNS` has no external (test) referent and was dropped from `pub` to `pub(crate)`.