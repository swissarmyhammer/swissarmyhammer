---
assignees:
- claude-code
depends_on:
- 01KRXHPVRP4XXABKDFHJ3NDWFJ
- 01KRXHC02J8GQNDSK91J9NDWN8
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffff180
title: 'claude-agent: persist SessionRecord via shared SessionStore and implement session/list'
---
Wire `claude-agent` onto the shared `SessionStore` / `SessionRecord` from `agent-client-protocol-extras`.

## Context — claude-agent has NO working durable persistence today
Production builds `SessionManager::new()` (`agent.rs:155`) with no storage path. `save_session_to_disk` / `load_session_from_disk` / `with_storage_path` exist but are test-only — production sessions live in an in-memory `HashMap` and are lost when the process exits. `loadSession: true` is advertised, but `session/load` only works within a single process lifetime. This card gives claude-agent durable, cross-restart persistence for the first time.

## Record persistence
- `Session.context: Vec<Message>` already wraps ACP `SessionUpdate`s — maps almost directly onto `SessionRecord.updates`. Build a `SessionRecord` from the live session and persist via `SessionStore` on each turn (or on a change threshold).
- Populate `title` / `updated_at` / `cwd` / `mcp_servers`.

## session/list
- Implement the `session/list` handler backed by `SessionStore::list`, with `cwd` filter and cursor pagination.
- Advertise `sessionCapabilities.list` in `initialize`. claude already advertises `load_session(true)` (`agent.rs:323`) — add list/resume alongside it.

## Delete dead code
- Remove the unused disk-persistence path: `save_session_to_disk` / `load_session_from_disk` / `delete_session_from_disk` / `with_storage_path` in `session.rs`. `SessionManager` stays as the in-memory live-session cache; durable persistence is now `SessionStore`.

## Verify
- `session/list` returns persisted sessions and round-trips through an actual process restart — this is new behavior that cannot work today.
- claude-agent test suite green.

Depends on the shared session-record card and the claude-agent RawMessageManager migration card.

## Review Findings (2026-05-18 14:18)

### Nits
- [x] `crates/claude-agent/src/agent_trait_impl.rs:328` — `persist_session_record` runs only on the happy path of `prompt`. The early returns above it — pre-cancelled session (`check_cancelled_before_processing`, ~line 271) and turn-limit hit (`check_turn_limits`, ~line 283) — return before reaching the persist call, even though `send_user_message_chunks` has already grown the live session's `context`. The dropped state is recovered by the next successful turn on the same session, so this is not data loss in practice, and the card scopes persistence as "on each turn (or on a change threshold)". Optional: persist before those early returns too, or document that cancelled/turn-limited turns are intentionally not persisted until the next successful turn. RESOLVED: `persist_session_record` is now called before both early-return paths (cancelled and turn-limited) so a cancelled or limited turn still durably records the context that was already accumulated.