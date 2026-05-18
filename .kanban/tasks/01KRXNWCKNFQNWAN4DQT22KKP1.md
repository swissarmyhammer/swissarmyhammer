---
assignees:
- claude-code
depends_on:
- 01KRXHVKSFAKZAVJ3W8TM95XQ6
- 01KRXHVR4ZZZ436ZGE85TVEG10
position_column: todo
position_ordinal: '8980'
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