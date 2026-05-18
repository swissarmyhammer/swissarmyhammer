---
assignees:
- claude-code
depends_on:
- 01KRXHPVRP4XXABKDFHJ3NDWFJ
- 01KRXHC02J8GQNDSK91J9NDWN8
position_column: todo
position_ordinal: '8580'
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