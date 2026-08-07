---
assignees:
- claude-code
position_column: todo
position_ordinal: ff9980
title: mirdan lib tests fail at random under parallel execution
---
`cargo test -p mirdan --lib` fails with a different set of tests on each run. It passes every time with `--test-threads=1`.

Measured on commit f0f12ae9a:
- run 1: 432 passed, 0 failed
- run 2: 423 passed, 9 failed — `install::tests::test_e2e_plugin_install_list_uninstall`, `list::tests::test_run_list_agent_filter_suppresses_validators`, `list::tests::test_run_list_no_filter_shows_validators`, `new::tests::test_new_plugin_creates_structure`, `new::tests::test_new_skill_creates_structure`, `new::tests::test_new_skill_already_exists`, `new::tests::test_new_tool_creates_structure`, `new::tests::test_new_validator_creates_structure`, `sync::tests::test_sync_validator_present_in_project_dir`
- `--test-threads=1`: 432 passed, 0 failed
- each failing test passes when run alone

The set of failures changes between runs, and every one of these tests writes or reads a working directory. The probable cause is shared process state — the current working directory — that the tests change without a guard.

Fix with the project pattern for this: a `CurrentDirGuard` RAII guard, or `serial_test`, on each test that changes the working directory. Do not add a production API to work around a test environment problem.

This is pre-existing. It was found while shipping the dead-code tool rules (^teemmch) and is unrelated to that change.

#tool-validators