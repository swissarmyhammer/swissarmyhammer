---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff9980
project: local-review
title: 'feat(llama-agent): make the GPU lock observable via tracing'
---
## What

`crates/llama-agent/src/gpu_lock.rs` (the machine-wide cross-process flock that serializes GPU generation) currently emits zero tracing. When diagnosing a real review run we cannot tell from `.sah/mcp.<pid>.log` whether cross-process GPU serialization is happening or how long a process waited.

Add tracing to `GpuLock::acquire_blocking` and `GpuLockGuard::drop` in `crates/llama-agent/src/gpu_lock.rs`:

- [x] `tracing::info!` when a process starts waiting for the lock, including the lock file path (before `FileExt::lock_exclusive`).
- [x] `tracing::info!` when the lock is acquired, including the wait duration (measure with `std::time::Instant` around `lock_exclusive`).
- [x] `tracing::debug!` on release in `GpuLockGuard::drop`.
- [x] Emit full payloads — never truncate the path or any message.

`tracing` is already a workspace dependency of llama-agent (queue.rs uses `info!`/`warn!`/`debug!`); follow the same import style.

## Acceptance Criteria

- [x] Running any generation that takes the GPU lock writes "waiting" and "acquired" lines (with lock path and wait duration) to the tracing log, and a debug line on release.
- [x] No behavior change to locking itself — existing gpu_lock tests (`second_acquirer_waits_until_first_releases`, `lock_auto_releases_when_holder_killed`) still pass.

## Tests

- [x] New unit test in the `tests` module of `crates/llama-agent/src/gpu_lock.rs` using `#[tracing_test::traced_test]` (the established repo pattern, e.g. `crates/swissarmyhammer-validators/src/review/fleet.rs`): acquire via `GpuLock::at_path` with a tempdir path, assert via `logs_contain` that the emitted events include the lock path string; drop the guard and assert the release event. Must run in milliseconds — temp path only, never the real machine-wide lock, no model. *(Note: `traced_test` is unusable in this binary — its macro `expect`s on `set_global_default`, and the chat_template suite installs the global dispatcher via `try_init()`, so registration order is a race. Verified against tracing-test-macro 0.2.6 source. The test uses a scoped subscriber with the shared `swissarmyhammer_common::test_utils::CaptureWriter` instead and asserts the same `logs_contain`-style contracts.)*
- [x] `cargo test -p llama-agent gpu_lock` — all green, <10s.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

## Review Findings (2026-06-10 14:42)

### Warnings
- [x] `crates/llama-agent/src/gpu_lock.rs:110` — acquire_blocking propagates bare io::Error from three distinct operations (create_dir_all, open, lock_exclusive) with no context. A caller surfacing the error sees only e.g. 'permission denied' with no indication of which step failed or which lock path was involved — and this is library code, where the rule calls for typed errors that preserve that information. Introduce a thiserror enum (e.g. GpuLockError with CreateDir/Open/Lock variants, each carrying the PathBuf and the source io::Error) and return Result<GpuLockGuard, GpuLockError>, or at minimum wrap each step with io::Error::new including the operation and self.lock_path. *(Done: `GpuLockError` thiserror enum with CreateDir/Open/Lock variants, each carrying the lock `PathBuf` and source `io::Error`; `acquire_blocking` now returns `Result<GpuLockGuard, GpuLockError>`. queue.rs needed no change — it maps the error via `format!("{}", e)`, which is Display-generic. TDD: `acquire_error_names_failing_step_and_lock_path` watched RED on the bare OS error first.)*
- [x] `crates/llama-agent/src/gpu_lock.rs:172` — The test-local `Capture` struct (Arc<Mutex<Vec<u8>>> + io::Write + MakeWriter returning self.clone()) is the fourth byte-identical in-memory tracing-capture writer in the workspace — `SharedLogBuffer` already exists in this same crate (crates/llama-agent/tests/integration/streaming_generation.rs:162), plus `LineWriter` (crates/swissarmyhammer-tools/src/mcp/unified_server.rs:1182) and `BufferWriter` (crates/swissarmyhammer-tools/tests/review_global_subscriber.rs:58). This is past the rule-of-three threshold: each copy is fixed and improved separately instead of once. Hoist one canonical capture writer into `swissarmyhammer_common::test_utils` (e.g. `CaptureWriter` with a `contents() -> String` accessor) and use it here and in the three existing sites, instead of adding a fourth private copy. *(Done: `CaptureWriter` (with `contents()`/`contains()`) hoisted into `swissarmyhammer_common::test_utils` — traced_test genuinely cannot cover the gpu_lock assertion (its macro panics if chat_template's `try_init()` wins the global-dispatcher race), so the shared helper was warranted. All four sites now use it; the local `Capture`, `SharedLogBuffer`, `LineWriter`, and `BufferWriter` duplicates are deleted.)*

### Nits
- [x] `crates/llama-agent/src/gpu_lock.rs:94` — at_path takes a concrete PathBuf, forcing callers holding a &Path, String, or &str to convert explicitly at every call site. Accept `impl Into<PathBuf>` (kept owned internally, so no extra allocation for callers that already have a PathBuf): `pub fn at_path(lock_path: impl Into<PathBuf>) -> Self`.
- [x] `crates/llama-agent/src/gpu_lock.rs:103` — acquire_blocking is a fallible public API but its doc comment has no `# Errors` section; the blocking semantics are well documented while the failure modes (lock-dir creation, file open, flock failure) are not. Add an `# Errors` section listing the conditions: failure to create the parent directory, failure to open/create the lock file, and failure of the flock syscall.
- [x] `crates/llama-agent/src/gpu_lock.rs:240` — Hardcoded 200ms lock-hold duration configures the timing of the second-acquirer test; an unnamed literal makes it unclear this is a deliberate hold window (long enough for the background thread to park) versus an arbitrary sleep. Name it, e.g. `const HOLD_BEFORE_RELEASE: Duration = Duration::from_millis(200);` with a comment that it gives the background acquirer time to block on the flock.
- [x] `crates/llama-agent/src/gpu_lock.rs:294` — Hardcoded 20ms polling interval for the child-ready sentinel is an unexplained timing constant; paired with the 5s deadline on line 291 it implicitly defines a retry budget (~250 polls) that is invisible as written. Extract `const READY_POLL_INTERVAL: Duration = Duration::from_millis(20);` next to the deadline constant so the wait loop's timing parameters are named together. *(Done: `CHILD_READY_DEADLINE` and `READY_POLL_INTERVAL` named together.)*

## Implementation Notes (2026-06-10, review-findings pass)

- Also fixed a real flake surfaced while verifying: the tracing-assertion test intermittently captured nothing when run in parallel with the other gpu_lock tests. Root cause (tracing-core 0.1.36): with a single registered dispatcher, `Rebuilder::JustOne` evaluates a callsite's first-touch interest against the *touching thread's* default; a parallel lock test's background thread first-touching the shared `info!` callsite caches `Interest::never` globally. Fix: `#[serial_test::serial(gpu_lock)]` on the three lock tests that hit those callsites (serial_test was already a dev-dep). Verified 10/10 consecutive green runs (previously ~1 in 4 failed).
- Pre-existing, unrelated failure discovered and filed as task 01KTSJJMGSHQYN5XXDBCJYGZWS: `swissarmyhammer-common` `test_isolated_test_environment_drop_restores_home` fails under a parallel `test_utils` run on main (reproduced 3/3 without these changes).