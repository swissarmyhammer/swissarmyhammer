---
assignees:
- claude-code
position_column: todo
position_ordinal: 8f80
title: Fix llama-agent's unbounded ACP session map; align session-cleanup policy
---
The two agents disagree on in-memory session lifecycle: one expires, one leaks.

## Current state
- claude-agent: `SessionManager` expires sessions — `cleanup_interval` (5 min), `max_session_age` (1 hr), `cleanup_expired_sessions`.
- llama-agent: the ACP-layer `sessions` and `llama_to_acp` maps (`crates/llama-agent/src/acp/server.rs`) have NO removal path — they grow unbounded for the entire process lifetime.
- Neither agent implements `session/close`.

## Target
- llama's ACP session maps get a cleanup/eviction path consistent with claude's `SessionManager`.
- Decide whether the eviction policy itself should be shared. Once `SessionStore` (persistent records) lands, the in-memory maps become caches over the store — evicting a cache entry is safe because the durable record persists. Align both agents on: in-memory map = bounded cache; durable truth = `SessionStore`.
- `session/close` is unimplemented in both — out of scope here unless explicitly wanted; flag it.

## Verify
- llama's ACP session maps do not grow without bound over a long-lived process.
- Eviction does not lose a session — it remains resolvable from `SessionStore`.

Overlaps card 5 (SessionStore) and cards 6/8 (which retire the old persistence) — coordinate.