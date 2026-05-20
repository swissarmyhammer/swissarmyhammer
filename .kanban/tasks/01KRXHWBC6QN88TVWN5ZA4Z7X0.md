---
assignees:
- claude-code
depends_on:
- 01KRXHVR4ZZZ436ZGE85TVEG10
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffff480
title: 'llama-agent: implement session/resume (primary) and rewire session/load via chat-template re-render'
---
Implement `session/resume` (the primary goal) and rewire `session/load` for `llama-agent`, both backed by chat-template re-render.

## ResumeStrategy — state restoration
- Implement `ResumeStrategy::restore` for llama-agent: convert `SessionRecord` -> `Vec<Message>`, run them through `chat_template.rs` to produce the fully rendered prompt text ("all the rendered text"), and prime the model so the next prompt continues the conversation.
- Unlike claude-agent, the record IS the resume input — no external process holds state.

## session/resume — NEW, primary goal
- Add a `session/resume` handler (`ResumeSessionRequest` -> `ResumeSessionResponse`), dispatched alongside the existing `Req::LoadSessionRequest` arm in `acp/server.rs`. Restore state via `ResumeStrategy::restore`, then return. MUST NOT replay history.
- Advertise `sessionCapabilities.resume` in `initialize`.
- Net-new wiring.

## session/load — rewire existing (it already replays)
`load_session` (`acp/server.rs:546`) is NOT a stub despite its doc-comment — it already replays the full history via `session/update` notifications, and the empty `LoadSessionResponse::new()` is the correct (empty) response.
- Rewire it to source from `SessionStore::load` -> `SessionRecord` and use the extracted conversion function, instead of reading `llama_session.messages` directly and converting inline.
- Add an explicit `ResumeStrategy::restore` call so the model is primed — today's load restores the session into the session manager but does not explicitly re-render/prime.
- Fix the stale "stub implementation" doc-comment.

## Verify
- After a restart: `session/resume` restores state and a follow-up prompt produces output consistent with the restored context, with NO replay to the client.
- `session/load` replays the full history to the client, then continues.
- Test both paths explicitly. llama-agent suite green, including `acp_integration.rs`; add `session/resume` coverage.

Depends on the llama-agent session-record card.

## Review Findings (2026-05-18 14:55)

### Warnings
- [x] `crates/llama-agent/tests/acp_integration.rs` (`with_temp_state`) — The helper sets the process-global `XDG_STATE_HOME` env var but never restores its prior value. After each test it leaves `XDG_STATE_HOME` pointing at a now-deleted temp dir. `#[serial]` prevents concurrent races, but the leaked mutation persists for the rest of the test binary's run and can affect other serialized tests (now or later) that read `XDG_STATE_HOME` without going through this helper. The symmetric claude-agent helper (`crates/claude-agent/tests/integration/session_resume.rs:59`) captures the previous value with `std::env::var_os` and restores it (`set_var`/`remove_var`) after the body runs. Mirror that: capture and restore the prior `XDG_STATE_HOME` so each test leaves the env as it found it.

### Nits
- [x] `crates/llama-agent/tests/acp_integration.rs` (`with_temp_state` doc comment) — The doc comment reads "Run `body` with `XDG_STATE_HOME` pointed at a fresh temp directory" and "The temp directory is returned alongside so it outlives `body`", but this function takes no `body` parameter — it only sets the env var and returns the `TempDir` guard. The wording was copied verbatim from the claude-agent closure-taking `with_temp_state` and not adapted. Reword to describe the actual contract (sets `XDG_STATE_HOME`, returns a `TempDir` RAII guard the caller binds to keep the dir alive).