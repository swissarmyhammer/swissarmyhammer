---
assignees: []
position_column: todo
position_ordinal: fb80
title: Fix flaky test_execute_use_command_with_temp_config (cwd race)
---
**File**: swissarmyhammer-cli/src/commands/model/use_command.rs:295-353

**Symptom**: `commands::model::use_command::tests::test_execute_use_command_with_temp_config` fails intermittently in workspace `cargo test --workspace`. Passes in isolation (single test, or single-threaded run, or only running model command tests).

**Root cause**: Test mutates process-global current working directory via `env::set_current_dir(&temp_path)`. It uses `#[serial_test::serial]` which only serializes against other `#[serial_test::serial]` tests. Other tests in the workspace that run in parallel and read or set cwd race with this test.

**Status on this branch**: Pre-existing flake — file is unchanged on the `avp` branch (last commit b7f1c5a80 on main). Not caused by the avp-common /implement integrations.

**What was tried**:
- Re-running in isolation (`cargo test -p swissarmyhammer-cli --lib commands::model::use_command::tests::test_execute_use_command_with_temp_config`): passes
- Running all model command tests with `--test-threads=1`: passes
- Running full workspace: fails reliably

**Suggested fix**: Replace `env::set_current_dir` + `serial_test::serial` with a non-cwd-mutating approach. The function under test should accept the directory as a parameter (or read it from a config struct) rather than relying on cwd. If that's not feasible, use the project's `CurrentDirGuard` pattern that quarantines all cwd-mutating tests under one `serial_test` group across the entire workspace.

Tag: test-failure #test-failure