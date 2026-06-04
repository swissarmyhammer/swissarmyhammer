---
assignees:
- claude-code
depends_on:
- 01KT9X9XM3BNT2GZRHYM4RH9VD
- 01KT9X8J16F6K88TVE3A628Y10
- 01KT9X9C6B7DXMN6PW84Q1S1CS
position_column: todo
position_ordinal: '8580'
project: claude-hooks
title: Wire HookableAgent into the llama ACP server lifecycle (session/prompt/stop)
---
`HookableAgent` and its lifecycle helpers exist but are never instantiated by the llama agent (`grep` shows zero uses outside the extras crate). Wire them into `crates/llama-agent/src/acp/server.rs` for the non-tool lifecycle seams. (The tool-dispatch seam is a separate task.)

## Design decisions
- Hooks are cwd-scoped: user `~/.claude` is global but project `.claude` depends on the SESSION cwd, and ACP sessions each carry a cwd. So load the HookConfig PER SESSION using the session's cwd (via the loader task) and build registrations once per session. Cache per session_id. Decide where the per-session hook state lives (alongside the session record / session_mcp_clients map).
- Build registrations with `hookable_agent_from_config(.., Some(LlamaHookEvaluator))` so prompt/agent hooks work. Provide `HookCommandContext` (transcript_path if available, permission_mode string from the server's permission mode).
- The AcpServer drives the connection via `connect_with` (SDK 0.11), not the `ConnectTo` middleware, so fire hooks by calling the helper methods directly at the seams — do NOT try to insert HookableAgent as transport middleware.

## Seams to wire
- `new_session` / `load_session` (server.rs ~1105): after success, call `track_session_start(session_id, Startup|Resume, cwd)` → fires SessionStart hooks (and records cwd for later events).
- `prompt` (server.rs:2081) entry: call `run_user_prompt_submit(request)`; on Err (Block/Cancel) return the ACP error and do not run the turn; otherwise use the possibly context-injected request.
- `prompt` return: call `run_stop(session_id, response)`. If a Stop hook yields ShouldContinue, decide loop behavior — minimum: propagate the `hook_should_continue`/`hook_reason` meta on the response (already implemented in the helper). Optionally (note as follow-up) re-enter the agentic loop with the reason as feedback.
- Keep `intercept_notifications` ONLY for the `Notification` event family (agent_message/thought/plan/etc.); PreToolUse/PostToolUse move to the real-seam task.

## Acceptance criteria + tests (scripted fake model)
- SessionStart command hook fires once per new_session with source=startup, and on load_session with source=resume.
- A UserPromptSubmit command hook exiting 2 (or returning decision:block) blocks the prompt: the model is never invoked and the client gets the block reason.
- A UserPromptSubmit hook returning additionalContext prepends that context to the prompt the model sees.
- A Stop hook returning decision:block (=> ShouldContinue) annotates the response meta with hook_should_continue=true.
- Sessions with no .claude settings behave exactly as today (no hooks, no overhead beyond one cheap load).