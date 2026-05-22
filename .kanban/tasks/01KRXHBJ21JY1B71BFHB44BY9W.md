---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffed80
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

## Review Findings (2026-05-18)

Code review outcome: 1 finding (Low). Build, clippy, and all 5 tests pass. Design, correctness, and acceptance-criteria coverage are sound — the no-mutex single-writer design was correctly chosen, `xdg_state_dir` reuses existing plumbing, and the ULID constructor matches spec.

- [x] **[Low] Env-var tests lack `#[serial]` isolation.** `test_acp_session_dir_resolves_and_creates` and `test_new_writes_to_session_raw_jsonl` in `crates/agent-client-protocol-extras/src/raw_messages.rs` mutate the process-global `XDG_STATE_HOME` env var. Rust runs tests within one binary on parallel threads, so these two tests race each other (and any future env-var test in the crate). The sibling `swissarmyhammer-directory` crate already marks every env-mutating test `#[serial]` (via the `serial_test` crate) for exactly this reason — see the project `test-isolation-raii` convention. Fix: add `serial_test` as a dev-dependency to `agent-client-protocol-extras/Cargo.toml` and annotate both tests with `#[serial]`. They pass today only because the crate currently has few env tests; this is latent flakiness. RESOLVED 2026-05-18: added `serial_test` dev-dependency (workspace `3.4`) to `agent-client-protocol-extras/Cargo.toml`; annotated both env-mutating tests with `#[serial]`. The other three tests in the file use only `tempfile` paths and touch no env vars, so they were left unannotated. All 5 `raw_messages` tests pass; `cargo clippy -p agent-client-protocol-extras --all-targets` is clean.