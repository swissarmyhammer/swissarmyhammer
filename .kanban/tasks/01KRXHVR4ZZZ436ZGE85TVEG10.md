---
assignees:
- claude-code
depends_on:
- 01KRXHPVRP4XXABKDFHJ3NDWFJ
- 01KRXHC509V2E9ZJDJTR5JH1P7
position_column: todo
position_ordinal: '8680'
title: 'llama-agent: SessionUpdate<->Message conversion, adopt shared SessionStore, implement session/list'
---
Wire `llama-agent` onto the shared `SessionStore` / `SessionRecord`.

## Conversion — extract the existing inline conversion, add the reverse
A `Message` -> `SessionUpdate` conversion already exists, inline, in `load_session` (`acp/server.rs:618-644`): User -> `UserMessageChunk`, Assistant -> `AgentMessageChunk`, Tool -> `AgentMessageChunk` (lossy — tool results crammed into an agent text chunk), System -> skipped.
- Extract that into a reusable function.
- Fix the lossy Tool handling: tool calls/results should round-trip as proper ACP tool-call updates, not collapse into agent text.
- Add the reverse direction: `SessionRecord.updates` -> `Vec<Message>` for restore.
- Handle `compaction_history`: the record should still replay a coherent conversation to the client; decide whether compacted turns appear in `updates` or only post-compaction messages.

## Record persistence + list
- Persist `SessionRecord` via `SessionStore`. llama-agent DOES have working durable persistence today (`FileSessionStorage` in `storage.rs`, auto-save plumbing in `session.rs`) — this is a real migration, not a first-time addition (unlike claude-agent).
- Implement the `session/list` handler backed by `SessionStore::list`.
- Advertise `sessionCapabilities.list` in `initialize`.

## Retire
- Remove `FileSessionStorage` and the `SessionStorage` trait, or reduce them to a thin adapter over `SessionStore`.

## Verify
- Round-trip messages -> record -> messages is loss-free for a representative conversation, including tool calls.
- llama-agent test suite green, including `storage` tests.

Depends on the shared session-record card and the llama-agent RawMessageManager migration card.