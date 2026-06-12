---
assignees:
- claude-code
depends_on:
- 01KT9X9XM3BNT2GZRHYM4RH9VD
- 01KT9X8J16F6K88TVE3A628Y10
- 01KT9X9C6B7DXMN6PW84Q1S1CS
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffed80
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

## Review Findings (2026-06-04 15:11)

Scope: working-tree changes for the lifecycle-wiring task (`crates/llama-agent/src/acp/hooks.rs`, the session/prompt/stop seams in `crates/llama-agent/src/acp/server.rs`, and the `LlamaHookEvaluator` it instantiates). Supporting code owned by the dependency tasks (hook_config / hookable_agent / hook_settings / json.rs) was read for context but reviewed only where this task depends on its contract. Build, clippy (`-p llama-agent --all-targets`), the 6 `hook_lifecycle` lib tests, the 9 `hook_evaluator` integration tests, and the 47 extras `hookable` tests all pass.

### Warnings
- [x] `crates/llama-agent/src/acp/hooks.rs` `build_agent` — `HookCommandContext.transcript_path` is left at its empty-string default; only `with_permission_mode` is set. The design notes call for "transcript_path if available," and a per-session transcript path *is* available — `wire_raw_message_manager` (server.rs ~896) builds `<acp-session-dir>/raw.jsonl` from the session ULID via `RawMessageManager::new`, and in `new_session` it runs (server.rs ~2003) immediately before `track_session_start` (~2008). Command hooks therefore see no transcript path, diverging from Claude Code's hook contract where a hook can read the transcript. Thread the session's raw transcript path into `build_agent` (it would need the `session_id`/path alongside `cwd`) and pass it via `with_transcript_path`. If intentionally deferred, note it as an explicit follow-up rather than a silent gap.
  - RESOLVED: Added `raw_transcript_path(session_ulid)` to `agent-client-protocol-extras` (extracted from `RawMessageManager::new`, re-exported) so the path is derived through one helper. `SessionHooks::track_session_start` now resolves the session's `raw.jsonl` from the session id and threads it into `build_agent(cwd, transcript_path)`. Discovered the deeper gap: the wrapper's `command_context` (set by `with_permission_mode`/`with_transcript_path`) never reached the command/prompt/agent handlers — they serialized stdin via `to_command_input()` (default ctx). Completed the documented-but-unwired contract: added `HookConfig::build_registrations_with_context` and `hookable_agent_from_config_with_context`, capturing the `HookCommandContext` into each handler so command-hook JSON stdin now carries `transcript_path` and `permission_mode`. New tests: `command_hook_input_carries_transcript_path` (server lib, asserts the SessionStart command hook's stdin carries the exact resolved `raw.jsonl` path) and `command_handler_stdin_carries_context_transcript_path` (extras lib, asserts the handler folds the context into stdin).

### Nits
- [x] `crates/llama-agent/src/acp/hooks.rs` `build_agent(&self, cwd: &PathBuf)` — takes `&PathBuf` where `&Path` is the idiomatic borrow (clippy's `ptr_arg` does not fire here, but `&Path` is the convention used elsewhere in this module, e.g. `load_hook_config(cwd)` and `permission_mode_string(&PermissionPolicy)`).
  - RESOLVED: `build_agent` now takes `cwd: &Path` (and `transcript_path: Option<&Path>`).
- [x] `ARCHITECTURE.md` section 3 (Agents) — the new Claude-compatible hook lifecycle (SessionStart/UserPromptSubmit/Stop wired into the llama ACP server) is a contained extension of the existing ACP layer, not a new crate or dependency edge, so no boundary is violated. Consider a one-line mention in the ACP subsection once the full hooks feature (incl. the tool-dispatch seam) lands, so the doc reflects that llama-agent fires `.claude/settings.json` hooks at lifecycle seams.
  - RESOLVED: Added a paragraph to the ACP subsection of section 3 describing the per-session `HookableAgent` firing `SessionStart`/`UserPromptSubmit`/`Stop` from `.claude/settings.json`, noting it is a contained ACP-layer extension and that the tool-dispatch seam lands in a separate task.