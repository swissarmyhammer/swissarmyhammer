---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffb680
project: acp-upgrade
title: Fix avp-common context recording tests broken by ACP 0.11 reshape
---
## What

Two `avp-common` tests in `avp-common/src/context.rs` panic after the ACP 0.11 migration — both relate to the recording flush + session-id propagation surfaced through the new Agent reshape.

### Failing tests

1. `context::tests::test_recording_is_always_on_with_no_env_vars` — panics at `context.rs:1848`:
   ```
   recordings dir must exist after a context lifetime, looked at /private/var/folders/.../.avp/recordings
   ```
   Root cause hypothesis: the always-on recording path no longer flushes a recording file to `<git_root>/.avp/recordings` on `AvpContext` drop when constructed with a `PlaybackAgent`. After the `3b8037fa6 ACP 0.11: avp-common: context.rs production Agent reshape` commit, the eager-arm Agent wiring may have broken the Drop-side flush.

2. `context::tests::test_set_session_id_propagates_through_eager_with_agent` — panics at `context.rs:1793`:
   ```
   read recording dir: Os { code: 2, kind: NotFound, message: "No such file or directory" }
   ```
   Same shape: the recording dir is never created because nothing writes to it before the test reads `std::fs::read_dir`.

Both tests pass on `main` (pre-migration baseline) and fail on `acp/0.11-rewrite` after the avp-common reshape. They are NOT environmental flakes — they fail deterministically.

## Acceptance Criteria

- [x] `cargo nextest run -p avp-common context::tests::test_recording_is_always_on_with_no_env_vars` passes.
- [x] `cargo nextest run -p avp-common context::tests::test_set_session_id_propagates_through_eager_with_agent` passes.
- [x] No regression in the rest of the avp-common test suite.

## Tests

- The two failing tests above are the regression tests — make them green.
- Also run the full `cargo nextest run -p avp-common` to confirm nothing else broke.

## Workflow

- Investigate whether the `RecordingAgent` wrapper now needs explicit flushing on Drop, or whether the eager-arm constructor is short-circuiting the recording wrap.
- Cross-reference with `8a05d6c2b ACP 0.11: avp-common: validator/runner.rs mock Agent + RecordingAgent wiring` and `3b8037fa6 ACP 0.11: avp-common: context.rs production Agent reshape`.

## Resolution

Root cause: the `RecordingAgent` wrapper is moved into the spawned `connect_with` task at arm time, so the recording state is owned by the task's future. Aborting the task via `JoinHandle::abort()` is asynchronous — it merely *signals* cancellation. On a single-threaded `#[tokio::test]` runtime, a synchronous caller (the test, which drops `AvpContext` and immediately reads the recordings dir) can race past the abort and observe an empty disk before the task's drop chain ever runs `RecordingState::drop`.

Fix:
- Added `pub struct RecordingFlushHandle` to `agent_client_protocol_extras::recording`. The handle holds an `Arc` clone of the same `RecordingState` the wrapper feeds, exposing a `pub fn flush(&self)` that calls `RecordingState::flush_now()` synchronously. New `RecordingAgent::flush_handle(&self)` returns one.
- In `avp-common/src/context.rs`, `arm_agent_connection` grabs a `flush_handle` from the wrapper *before* moving it into `connect_with`, stashes it on `ActiveAgent`, and a new `impl Drop for ActiveAgent` calls `recording_flush.flush()` ahead of the field-drop chain that aborts the task. The flush is best-effort and idempotent — re-flushing on shutdown is safe because `flush_now` marks the state clean.

Files changed:
- `agent-client-protocol-extras/src/recording.rs` — new `RecordingFlushHandle` + `RecordingAgent::flush_handle`.
- `agent-client-protocol-extras/src/lib.rs` — re-export `RecordingFlushHandle`.
- `avp-common/src/context.rs` — `ActiveAgent::recording_flush`, `arm_agent_connection` plumbing, `Drop for ActiveAgent`.

Verified: both target tests now pass; full `cargo nextest run -p avp-common` is 684 passed / 0 failed; `cargo nextest run -p agent-client-protocol-extras` is 253 passed / 0 failed; `cargo clippy -p avp-common -p agent-client-protocol-extras --all-targets` is clean.

## Depends on

- 01KQ36C3JQ5GKVYXAYW66J4H9H (workspace-wide green) — this task is the route-back for failures discovered there.