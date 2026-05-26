---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8f80
title: Fix `swissarmyhammer-tools` test parallel isolation — rotating failures from HOME/CWD/TempDir leakage
---
## What

The `swissarmyhammer-tools` lib test binary has parallel-execution test pollution: **a different test rotates as the failure each run**. Across recent invocations the failing test has been:

- `mcp::tools::shell::tests::test_dispatch_execute_command_empty_op`
- `mcp::tool_config::tests::test_watcher_deleted_file_reverts_to_all_enabled`
- `test_utils::tests::test_isolated_test_environment_drop_restores_home`

All three pass in isolation. The pattern is classic shared-global-state racing — HOME, CWD, env vars, a `OnceCell` / `Mutex` static, or shared `TempDir` paths leaking across threads in the same test binary.

## Root Cause Analysis (resolved)

Three distinct race conditions, all read-side. None of them was a missing mutator RAII restore — the mutators were already protected; the *readers* were unprotected.

### Race 1 — `test_dispatch_execute_command_empty_op`

`execute_op` (in `crates/swissarmyhammer-tools/src/mcp/tools/shell/test_helpers.rs`) calls into the shell tool with no `working_directory` arg. That falls through to `prepare_working_directory(None)` in `process.rs`, which reads `std::env::current_dir()` and then checks `work_dir.exists()`. Tests like `test_health_check_project_config_valid` use `CurrentDirGuard::new(tmp.path())` which sets CWD to a `TempDir`. Between this test's `current_dir()` read and the `work_dir.exists()` check, the other test can drop its guard and delete its `TempDir`, leaving us with `WorkingDirectoryError`. `TestCommandBuilder::new` already handles this by defaulting `working_directory` to `/tmp`; the dispatch helpers did not.

### Race 2 — `test_watcher_deleted_file_reverts_to_all_enabled`

`ToolConfigWatcher::check_and_reload` calls `load_merged_tool_config()` on reload. That function reads the *process-global* HOME via `dirs::home_dir()` and the process-global CWD via `find_git_repository_root()` — it ignores the watcher's stored paths for the actual load (the stored paths are only used for mtime tracking). If another parallel test had set HOME to a fake home that contains a `.sah/tools.yaml` disabling shell, or CWD to a workspace whose `.sah/tools.yaml` disabled shell, the reload would pick that up and the assertion `registry.is_tool_enabled("shell")` would fail.

### Race 3 — `test_isolated_test_environment_drop_restores_home`

The test read `HOME` *outside* of `HOME_ENV_LOCK`, then dropped an `IsolatedTestEnvironment`, then read `HOME` again. When another test was inside its own `IsolatedTestEnvironment` at the time of the first read, the test observed that other test's fake HOME, then later — after the other test had restored the *real* HOME — saw a different value and failed.

## Fix Summary

Smallest correct isolation, using existing primitives:

1. **`execute_op` / `run_command_with`** (`shell/test_helpers.rs`): default `working_directory` to `/tmp` for execute-command dispatches, matching `TestCommandBuilder::new`. Removes the dependence on a stable process CWD.
2. **`test_watcher_detects_file_change` and `test_watcher_deleted_file_reverts_to_all_enabled`** (`tool_config.rs`): hold an `IsolatedTestEnvironment` for the lifetime of the reload (gives HOME isolation via the existing global lock), and chdir into a clean temp dir under `#[serial_test::serial(cwd)]` using the canonical `CurrentDirGuard`.
3. **`test_isolated_test_environment_drop_restores_home`** (`test_utils.rs`): rewrite to atomically capture HOME under the `HOME_ENV_LOCK`, stamp a sentinel, drop the lock so `IsolatedTestEnvironment::new` can acquire it, drop the env, then re-acquire the lock to read the restored HOME. Also exposed `acquire_home_env_lock()` (test-utility, symmetric to the existing `acquire_semantic_db_lock()`).

No production API was added to fix test env problems — only test-utility additions and test-only isolation primitives.

## Acceptance Criteria
- [x] `cargo test -p swissarmyhammer-tools` is green for 10 consecutive full-suite runs.
- [x] Each previously-failing test still passes in isolation.
- [x] Isolation fixes use existing primitives (no new prod API to support tests).
- [x] No `#[ignore]` / `#[allow]` band-aids; no `--test-threads=1` workaround.
- [x] `cargo clippy -p swissarmyhammer-tools --all-targets -- -D warnings` clean.

## Tests
- [x] All three currently-known tests (`test_dispatch_execute_command_empty_op`, `test_watcher_deleted_file_reverts_to_all_enabled`, `test_isolated_test_environment_drop_restores_home`) pass in the full-suite invocation, repeatedly.
- [x] Whichever other tests mutate the shared state still pass.

## Files Changed
- `crates/swissarmyhammer-tools/src/mcp/tools/shell/test_helpers.rs` — default `working_directory` to `/tmp` in `execute_op` / `execute_op_with` / `run_command_with`.
- `crates/swissarmyhammer-tools/src/mcp/tool_config.rs` — add `IsolatedTestEnvironment` + `#[serial(cwd)]` + `CurrentDirGuard` to the two watcher tests that trigger an actual reload.
- `crates/swissarmyhammer-common/src/test_utils.rs` — expose `acquire_home_env_lock()`; rewrite `test_isolated_test_environment_drop_restores_home` to use the lock + sentinel pattern.

## Verification
- 10 consecutive runs of `cargo test -p swissarmyhammer-tools --lib --tests`: all green.
- Each named test passes in isolation.
- `cargo clippy -p swissarmyhammer-tools --all-targets -- -D warnings` clean.
- `cargo clippy -p swissarmyhammer-common --all-targets -- -D warnings` clean.

## Review Findings (2026-05-26 17:35)

### Warnings
- [x] `crates/swissarmyhammer-tools/src/mcp/tool_config.rs:294` — The watcher tests introduce a new one-off `CwdGuard(PathBuf)` instead of using the canonical `swissarmyhammer_common::test_utils::CurrentDirGuard` already imported in the same `mod tests` (via `IsolatedTestEnvironment`). This duplicates the established RAII primitive, contradicts the project's `test-isolation-raii` rule (which names `CurrentDirGuard` explicitly), and even diverges from this task's own description which promised "watcher tests now wrap `IsolatedTestEnvironment` + `CurrentDirGuard`". The canonical guard is strictly stronger: it validates the target dir exists before chdir, recovers from invalid CWD on entry, and falls back to `CARGO_MANIFEST_DIR` on restore failure — none of which the local `CwdGuard` does. Replace the local `CwdGuard` struct + manual `set_current_dir` calls in `test_watcher_detects_file_change` and `test_watcher_deleted_file_reverts_to_all_enabled` with `let _cwd = CurrentDirGuard::new(cwd_dir.path()).expect("chdir guard");` and delete the `CwdGuard` definition (lines 286-300). The `#[serial(cwd)]` attribute can stay — `CurrentDirGuard`'s internal lock is compatible with the serial token and provides defense in depth.

### Resolution (2026-05-26)
Replaced local `CwdGuard` struct + manual `std::env::set_current_dir` calls in both `test_watcher_detects_file_change` and `test_watcher_deleted_file_reverts_to_all_enabled` with `CurrentDirGuard::new(cwd_dir.path()).expect("chdir guard")`. Deleted the local `CwdGuard` definition (former lines 286-300). Import line in `mod tests` updated to `use swissarmyhammer_common::test_utils::{CurrentDirGuard, IsolatedTestEnvironment};`. `#[serial_test::serial(cwd)]` retained on both tests for defense in depth. Both tests pass; `cargo clippy -p swissarmyhammer-tools --all-targets -- -D warnings` is clean.
