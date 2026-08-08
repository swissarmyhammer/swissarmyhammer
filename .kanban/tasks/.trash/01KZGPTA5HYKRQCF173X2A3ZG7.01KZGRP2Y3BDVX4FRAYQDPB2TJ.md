---
assignees:
- claude-code
position_column: todo
position_ordinal: ffa680
title: Fix the mirdan CWD race that fails one test in half of all suite runs
---
`cargo test -p mirdan --lib` fails about half the time, and the test that fails changes from run to run. Six different tests have been seen to fail, every one of them in a module that changes the process working directory.

## Measurement

Eight consecutive runs of `cargo test -q -p mirdan --lib` on one machine: **4 failed, 4 passed**. Six runs with the working tree reverted to `HEAD` for `crates/mirdan/`: **1 failed, 5 passed**. The failure is therefore already on `main` and is not caused by any pending change.

The tests seen failing, each in a different run:

- `install::tests::test_install_tool_from_tool_md_content`
- `install::tests::test_e2e_all_four_types_coexist`
- `install::tests::test_deploy_and_uninstall_plugin`
- `install::tests::test_deploy_and_uninstall_tool`
- `list::tests::test_scan_scoped_store_reads_the_project_validator_directory`

## Root cause

The process working directory is global, and these test modules write it without holding the lock that would make that safe:

| file | `set_current_dir` calls | `#[serial]` markers |
|---|---|---|
| `crates/mirdan/src/install/tests.rs` | 44 | 29 |
| `crates/mirdan/src/list.rs` | 8 | 4 |
| `crates/mirdan/src/new.rs` | 12 | 6 |

Every one of these tests does the same three steps: read the current directory, move to a `tempfile::tempdir()`, and move back at the end. A test that is not `#[serial]` runs beside a `#[serial]` one, so two threads own the working directory at once. When the first test's temporary directory is deleted while a second test still stands in it, the second test's own `std::env::current_dir()` fails.

That is the reported error exactly. `install::tests::test_deploy_and_uninstall_tool` fails on its first line, `let old_dir = std::env::current_dir().unwrap();`, with `Io(Os { code: 2, kind: NotFound, message: "No such file or directory" })`. The directory the process stands in no longer exists.

The manual save-and-restore is the second half of the defect. `std::env::set_current_dir(old_dir).unwrap()` is the last statement of each test, so a test that panics earlier never runs it and leaves every later test standing in a deleted directory.

## Work

1. Replace every manual save/restore pair with `swissarmyhammer_common::test_utils::CurrentDirGuard`. It already exists, mirdan already depends on `swissarmyhammer-common`, and a guard restores the directory on the panic path too.
2. Put `#[serial]` on every test that changes the working directory. `serial_test` is already a dev-dependency of mirdan. The three files above are 64 call sites against 39 markers.
3. Prove the fix: run `cargo test -p mirdan --lib` twenty times and get twenty green runs. One green run proves nothing here — the failure rate is about one run in two.

## Wider check

`crates/swissarmyhammer-common/src/test_utils.rs::CurrentDirGuard` exists for exactly this. Search the workspace for other test modules that call `std::env::set_current_dir` directly and route them through the guard as well.