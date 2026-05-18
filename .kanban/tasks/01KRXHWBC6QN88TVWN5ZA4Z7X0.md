---
assignees:
- claude-code
depends_on:
- 01KRXHVR4ZZZ436ZGE85TVEG10
position_column: todo
position_ordinal: '8880'
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