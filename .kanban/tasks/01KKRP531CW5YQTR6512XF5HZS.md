---
assignees:
- claude-code
position_column: done
position_ordinal: ffffff9580
title: 'heb/store.rs: init_schema() called on every open_connection() path implicitly but schema is only explicit in init_schema()'
---
heb/src/store.rs:14-34 and heb/src/context.rs:34

`init_schema()` must be called before `log_event()` or `replay()` — if callers forget, they get a SQLite error. The public API does not enforce this ordering. `HebContext` calls `init_schema` in `open()` and also in the test helper, but `store::log_event` is also `pub` and can be called without any schema init (as demonstrated in the integration test `TestEnv::publish_to_db` which calls `init_db()` only sometimes).

The integration test `test_leader_promotion_resumes_bus` (line 310) calls `env.expect_leader()` and uses the bus without ever calling `env.init_db()`, which would fail if it tried to persist directly.

Suggestion: call `init_schema` inside `log_event` using `CREATE TABLE IF NOT EXISTS` (already idempotent — it already uses that). Remove the separate `init_schema` from the public API, or make it private and call it lazily from `log_event`. #review-finding