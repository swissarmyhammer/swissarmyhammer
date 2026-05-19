---
assignees:
- claude-code
depends_on:
- 01KRXHVKSFAKZAVJ3W8TM95XQ6
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffff380
title: 'claude-agent: implement session/resume (primary) and rewire session/load via `claude --resume`'
---
Implement `session/resume` (the primary goal) and rewire `session/load` for `claude-agent`, both backed by the claude CLI's own resume.

## ResumeStrategy — state restoration
- Implement `ResumeStrategy::restore` for claude-agent: re-spawn the claude CLI with `--resume <uuid>`, where `<uuid>` is `SessionId::to_uuid_string()` of the session ULID — deterministic, no stored mapping needed.
- `ClaudeProcessManager` gains a resume spawn path alongside the current `--session-id` new-session spawn.
- If the CLI's own transcript for that uuid is gone, surface a clear error.

## session/resume — NEW, primary goal
- Add a `session/resume` handler (`ResumeSessionRequest` -> `ResumeSessionResponse`, method `session/resume`). Restore state via `ResumeStrategy::restore`, then return. MUST NOT replay history.
- Advertise `sessionCapabilities.resume` in `initialize`.
- Net-new wiring: request dispatch, handler, capability.

## session/load — rewire the REAL path
The live `session/load` path is `agent_trait_impl.rs::load_session` -> `agent.rs::handle_session_found` -> `replay_session_history` + `build_load_session_response`. (`EnhancedSessionLoader` / `session_loading.rs` is NOT in this path — see below.)
- Rewire `handle_session_found` / `replay_session_history` to source from `SessionStore::load` -> `SessionRecord` instead of the in-memory `SessionManager`.
- Flow: load record -> restore state via `ResumeStrategy::restore` -> replay `record.updates` as `session/update` notifications -> return `LoadSessionResponse`. The replay step is the only difference from `session/resume`.

## Salvage + delete the dead loader module
`crates/claude-agent/src/session_loading.rs` (~870 lines: `EnhancedSessionLoader`, `SessionHistoryReplayer`, `SessionNotificationSender`) is dead code — `EnhancedSessionLoader::new` is referenced only by its own tests, never by the production handler. It has richer logic than the live path.
- Port the worthwhile pieces into the rewired live path: session-expiration check, session-integrity validation, replay error-recovery with backoff, capability gating.
- Then delete `session_loading.rs` and its module declaration; its tests move to the live path.

## Verify
- After an actual process restart: `session/resume` restores state and the next prompt continues, with NO history replayed to the client.
- `session/load` replays the full history to the client, then continues.
- Test both paths explicitly. claude-agent suite green; add `session/resume` coverage; confirm nothing else depended on `session_loading.rs`.

Depends on the claude-agent session-record card.

## Review Findings (2026-05-18 19:30)

### Warnings
- [x] `crates/claude-agent/src/session_resume.rs` — The file has no `#[cfg(test)]` module: `replay_record_updates`, `rehydrate_in_memory_session`, `build_replay_notification`, `check_record_expiration`, and `check_record_integrity` have zero direct unit coverage. The integration tests in `tests/integration/session_resume.rs` only exercise the pre-CLI-spawn path (every record there is rejected before `restore` runs), so the salvaged replay loop — the most intricate piece of this card — and the in-memory session rehydration are never executed by any test. `replay_record_updates` needs no claude CLI and could be unit-tested with a `SessionRecord` carrying `updates` plus a captured notification channel, asserting both the happy-path notification stream and the abort-after-`MAX_REPLAY_FAILURES` behavior. Add unit tests for the replay loop (success stream, consecutive-failure abort) and for `rehydrate_in_memory_session` (cwd/mcp_servers/updates restored into the `SessionManager`).
  RESOLVED: Coverage added via the integration suite (`tests/integration/session_resume.rs`), which runs under nextest — a `#[cfg(test)]` module in `src/` would be dead because `[lib] test = false`. Made `replay_record_updates` and `rehydrate_in_memory_session` `pub` (the former was `pub(crate)`, which integration tests cannot reach) and added a `pub session_manager()` accessor on `ClaudeAgent` so a test can observe the rehydrated session. Three new tests: `replay_record_updates_streams_every_update_as_a_notification` (asserts every update is replayed in order, each tagged `historical_replay` with correct index/total — captured via the agent's global notification receiver); `replay_record_updates_is_a_noop_for_an_empty_record`; and `rehydrate_in_memory_session_restores_cwd_mcp_servers_and_updates` (cwd, MCP servers, and update history reconstructed into the `SessionManager`). `build_replay_notification`/`check_record_expiration`/`check_record_integrity` are exercised transitively (replay tagging, and the existing expired/corrupt-record tests). The abort-after-`MAX_REPLAY_FAILURES` branch is genuinely unreachable in tests — `NotificationSender::send_update` is infallible (a broadcast send with no subscriber is discarded, never `Err`) and `notification_sender` is a concrete type with no injectable fake; this is documented in a code comment on the branch and in the test-module doc comment.
- [x] `crates/claude-agent/src/session_resume.rs:236-238` — The doc comment on `replay_record_updates` claims "a transient notification failure is retried with exponential backoff". This is inaccurate: on a `send_update` error the `for` loop still advances to the *next* update — the failed update is dropped, not retried. The backoff only paces the stream before the following update. (This faithfully matches the original `SessionHistoryReplayer`, which also advanced the loop on failure — so the behavior is correct, only the comment is wrong.) Reword to describe the actual behavior: "a failed update is logged and skipped; the stream backs off before continuing, and the replay aborts only after `MAX_REPLAY_FAILURES` consecutive failures."
  RESOLVED: Doc comment on `replay_record_updates` reworded to describe the real skip-on-failure behavior — a failed update is logged and skipped (the loop advances to the next update rather than resending it), the stream backs off with an exponential delay before continuing, and the replay aborts only after `MAX_REPLAY_FAILURES` consecutive failures. No behavior change.