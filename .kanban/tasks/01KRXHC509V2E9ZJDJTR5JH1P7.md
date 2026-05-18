---
assignees:
- claude-code
depends_on:
- 01KRXHBJ21JY1B71BFHB44BY9W
position_column: todo
position_ordinal: '8380'
title: Migrate llama-agent to shared per-session transcript manager
---
Switch `llama-agent` to the shared `RawMessageManager` from `agent-client-protocol-extras` and delete its local copy.

## Remove
- `crates/llama-agent/src/acp/raw_message_manager.rs` (including the redundant `Arc<Mutex<File>>`).
- `pub mod raw_message_manager;` and the `pub use raw_message_manager::RawMessageManager;` re-export in `crates/llama-agent/src/acp/mod.rs`.

## Lifecycle change
Currently the manager is created in the ACP server constructor (`crates/llama-agent/src/acp/server.rs:76`) with a fixed `.acp/transcript_raw.jsonl` path. Move creation to `new_session` handling so the file is `<acp-session-dir>/raw.jsonl`:
- On `new_session`, create a shared `RawMessageManager` for that session ULID and store it on the session (or register it keyed by root ULID, matching the shared registry model).
- The `raw_message_manager` field on the server (`server.rs:50`) becomes per-session rather than per-server; the `record` call site (`server.rs:410`) sources it from the active session.

## Verify
- `crates/llama-agent` test suite green, including `tests/acp_integration.rs`.
- Raw frames for a session land in `$XDG_STATE_HOME/acp/<session-ulid>/raw.jsonl`.

Depends on the shared-manager card.