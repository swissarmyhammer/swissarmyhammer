---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffea80
project: claude-hooks
title: HookEvaluator backed by the llama model (prompt + agent hooks)
---
`type: prompt` and `type: agent` hook handlers (already parsed by HookConfig) require an `Arc<dyn HookEvaluator>` at `build_registrations` time. None exists. Implement one backed by the llama model so prompt/agent hooks evaluate live.

## Background
The evaluator contract (crates/agent-client-protocol-extras/src/hook_config.rs):
```
async fn evaluate(&self, prompt: &str, is_agent: bool) -> Result<String, String>;
// expected response JSON: {"ok": true} or {"ok": false, "reason": "..."}
```
The handler templates substitute `$ARGUMENTS` with the hook input JSON before calling `evaluate`. `prompt` hooks are single-turn (is_agent=false); `agent` hooks are multi-turn with tool access (is_agent=true).

## Scope
- Implement a `LlamaHookEvaluator` (in llama-agent, e.g. under src/acp/) holding an `Arc<AgentServer>`.
  - Single-turn path: mirror `AgentServer::generate_session_title`'s short model-call pattern. Wrap the user's prompt with a system instruction requiring the model to reply with exactly `{\"ok\": bool, \"reason\"?: string}` JSON; parse defensively (treat unparseable as `{\"ok\": true}` per existing handler fallback).
  - Agent path (is_agent=true): run a bounded multi-turn loop with the session's tools. If a full multi-turn agent loop is too large for this task, implement single-turn now and file a follow-up for true multi-turn — but the prompt path must work.
- Respect the handler `timeout` (already enforced by the handler wrappers) and bound tokens/turns to avoid runaway evaluations.
- Expose construction so the server-wiring task can pass `Some(evaluator)` into `hookable_agent_from_config`.

## Acceptance criteria + tests (use the llama-coverage scripted fake model)
- A `Stop` prompt hook whose evaluator returns `{\"ok\": false, \"reason\": \"tests failing\"}` yields a ShouldContinue decision; `{\"ok\": true}` allows.
- A `UserPromptSubmit` prompt hook returning ok:false blocks the prompt with the reason.
- Evaluator failure / unparseable output → Allow (never crashes the turn).
- No real GPU/weights needed; driven by the scripted model.

## Review Findings (2026-06-04 13:56)

Verified: title_via_model refactor is behavior-preserving (generate_short receives the same TITLE_GENERATION_INSTRUCTION, first_user_message, and 32-token cap, with the identical User-then-System message ordering of the original inline body); generate_session_title still calls it unchanged. Defensive JSON normalization is sound — extract_decision validates the exact PromptHookResponse shape and first_json_object correctly tracks brace depth while skipping string contents/escapes (no underflow possible since scanning starts at the first `{`). The is_agent=true path reusing single-turn is acceptable given the task explicitly permitted deferral and follow-up 01KT9Z8FFW78R20F4G7WFV4M4A was filed. All acceptance criteria covered by tests driving the real HookConfig::build_registrations pipeline. Verified locally: 7/7 integration tests (tests/hook_evaluator.rs) pass, 7/7 module unit tests pass, `cargo clippy -p llama-agent --tests` clean.

### Nits
- [x] `crates/llama-agent/tests/hook_evaluator.rs:197` — `HookEventKind` is imported (line 18) but only "used" via `let _ = HookEventKind::Stop;` with a comment admitting it exists solely to silence an unused-import warning. This is backwards: remove `HookEventKind` from the import list on line 18 and delete lines 197-198 rather than adding a meaningless dead statement to an unrelated test. Fixed: dropped `HookEventKind` from the import and deleted the dead statement; `cargo test -p llama-agent --test hook_evaluator` 7/7 pass, `cargo clippy -p llama-agent --tests` clean.