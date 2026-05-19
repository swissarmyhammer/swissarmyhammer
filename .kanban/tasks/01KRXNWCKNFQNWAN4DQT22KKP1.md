---
assignees:
- claude-code
depends_on:
- 01KRXHVKSFAKZAVJ3W8TM95XQ6
- 01KRXHVR4ZZZ436ZGE85TVEG10
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffff680
title: Auto-generate session titles and emit the built-in SessionInfoUpdate (both agents)
---
Give sessions human-readable titles so `session/list` is browsable instead of a wall of ULIDs, using the built-in ACP metadata mechanism.

## Mechanism — built-in, no extensions
- Titles are set/updated via the standard `SessionUpdate::SessionInfoUpdate { title, updated_at, _meta }` variant (schema 0.12.0, `client.rs`), sent agent->client over the existing `session/update` notification channel. `MaybeUndefined` gives set / null-to-clear / leave-unchanged.
- The title is stored on `SessionRecord.title` and persisted by `SessionStore`, so `session/list` returns it for sessions that are not currently open.
- No custom `ext` method — `session_info_update` IS the built-in rename/title mechanism.

## Shared behavior — keep consistent across both agents
- Trigger: generate the title after the first meaningful exchange (first user prompt + first agent response).
- On generation and on any later change: update `SessionRecord.title` + `updated_at`, persist via `SessionStore`, and emit one `SessionInfoUpdate` notification.
- Bump `updated_at` on session activity so `session/list` recency ordering is correct.

## Per-agent generation source — the essential (thoughtful) difference
- claude-agent: prefer the claude CLI's own generated session summary if exposed via the CLI/transcript; otherwise generate from the first user prompt. Generation must not block the prompt response — run async and emit `SessionInfoUpdate` when ready.
- llama-agent: generate with a short model call ("title this conversation in <=6 words") after the first exchange — better quality than a first-N-words heuristic. Async; emit `SessionInfoUpdate` when ready. Heuristic truncation of the first user message is an acceptable fallback if a first-turn model call is too costly.

## Verify
- `session/list` shows meaningful titles, not ULIDs.
- A client receives a `session_info_update` after the first exchange and the title appears live.
- Title persists and round-trips through a restart.

Depends on the claude-agent and llama-agent session-record cards.

## Review Findings (2026-05-18 20:40)

### Warnings
- [x] `crates/claude-agent/src/agent.rs:2864`, `crates/claude-agent/src/agent.rs:2933` — The two new `#[tokio::test]` tests for title generation (`maybe_generate_session_title_sets_and_persists_title`, `maybe_generate_session_title_waits_for_first_exchange`) live in the `#[cfg(test)] mod session_record_tests` inside the crate's `[lib]`. `crates/claude-agent/Cargo.toml` sets `[lib] test = false`, so these modules never compile or run. The task adds these as the title-generation coverage for claude-agent, but they verify nothing — the green suite does not exercise them. Move the title-generation tests to `crates/claude-agent/tests/integration/` (e.g. `session_persistence.rs`, which is a real integration test target) so they actually run. RESOLVED: both tests moved to `crates/claude-agent/tests/integration/session_persistence.rs`; `maybe_generate_session_title` widened from `pub(crate)` to `pub` so the integration target can drive it. They run under nextest as `claude-agent::agent_tests integration::session_persistence::*`. The `[lib] test = false` line was left untouched.
- [x] `crates/llama-agent/src/acp/server.rs:845` (`generate_and_emit_title`) and `crates/llama-agent/src/agent.rs:1962` (`generate_session_title` / `title_via_model`) — llama-agent's title path has zero test coverage. RESOLVED: added 3 lib tests in `crates/llama-agent/src/acp/server.rs` `mod tests` — `generate_and_emit_title_uses_heuristic_when_model_unavailable` (deterministic heuristic-fallback: no model loaded, covers title stored + persisted + one `SessionInfoUpdate` emitted), `generate_and_emit_title_skips_without_first_exchange` (trigger guard), `generate_and_emit_title_skips_when_title_already_set` (once guard).
- [x] `crates/llama-agent/src/acp/server.rs:887-902` (`generate_and_emit_title`) — Title application is a non-atomic read-modify-write. RESOLVED: added `SessionManager::mutate_session` closure-style in-place update in `crates/llama-agent/src/session.rs`; `generate_and_emit_title` now mutates only `title` under the write lock, matching claude-agent's `apply_session_title`. Added `test_mutate_session_does_not_clobber_concurrent_change` proving a concurrent `add_message` survives the title mutation.
- [x] `crates/claude-agent/src/agent.rs:2110-2113` vs `:2142` — The `SessionInfoUpdate` notification carries an independent `SystemTime::now()` while the persisted record's `updated_at` derives from `session.last_accessed`. RESOLVED: `apply_session_title` now captures `session.last_accessed` from the same `get_session` call used for persistence and derives the notification timestamp from it, so the persisted record and the live notification carry the identical value.

### Nits
- [x] `crates/claude-agent/src/agent.rs:2138` — Doc comment on `session_record_from` linked the non-existent `generate_session_title`. RESOLVED: link now points to `maybe_generate_session_title`.
- [x] `crates/llama-agent/src/acp/server.rs:899` — `updated.updated_at = SystemTime::now()` was redundant. RESOLVED: dropped; the new `mutate_session` bumps `updated_at` itself (mirroring `update_session`).
