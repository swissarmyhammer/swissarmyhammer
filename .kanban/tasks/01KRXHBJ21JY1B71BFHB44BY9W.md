---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
title: Create shared per-session transcript RawMessageManager in agent-client-protocol-extras
---
Consolidate the two duplicate `RawMessageManager` implementations into one shared implementation in the `agent-client-protocol-extras` crate.

## Current state
- `crates/claude-agent/src/agent_raw_messages.rs` — manager + global static `RAW_MESSAGE_MANAGERS` registry keyed by root session ID; bare `std::fs::File`.
- `crates/llama-agent/src/acp/raw_message_manager.rs` — near-identical manager, no registry, redundant `Arc<Mutex<File>>` (only one writer task exists).

Both write append-only line-delimited JSON-RPC frames to `.acp/transcript_raw.jsonl` resolved against `current_dir()`.

## Target
A single `RawMessageManager` in `agent-client-protocol-extras`:
- mpsc channel + one background writer task, append mode, flush per write. No mutex around the file (one writer).
- Owns the root-session registry (the `RAW_MESSAGE_MANAGERS` map keyed by root session ULID) so subagents share their root agent's manager.
- Writes to `<acp-session-dir>/raw.jsonl`.

## Shared per-session directory helper
Provide `acp_session_dir(session_ulid) -> PathBuf` in the shared crate:
- Resolves `$XDG_STATE_HOME/acp/<session-ulid>/`, fallback `~/.local/state/acp/<session-ulid>/`.
- Created on demand. Reuse the XDG plumbing already in `swissarmyhammer-directory` (config/data/state base-dir helper) rather than re-rolling it.
- The session-record store (separate card) reuses this exact helper — raw trace (`raw.jsonl`) and session record (`session.json`) are siblings inside the per-session directory.

## Lifecycle
The `RawMessageManager` constructor takes the session ULID, not a fixed path — the manager is created at `new_session` time when the ULID is known, not at agent construction.

## Notes
- ULID session IDs are globally unique and lexically time-sortable, so the global directory is collision-free across projects.
- Keep the existing `record(message: String)` non-blocking API.
- Port the existing unit test for write/flush/read-back.

Prerequisite for the claude-agent / llama-agent migration cards and the shared session-record card.