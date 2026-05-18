---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
title: 'claude-agent: fix ~38 pre-existing session-storage test failures and a server::tests hang'
---
## What
`cargo test -p claude-agent` has **~38 pre-existing failures** outside the `tools` module, plus one hanging test. Confirmed pre-existing (reproduce on a clean tree with all uncommitted changes stashed) and unrelated to the plugin platform work — discovered during task 01KRXK1ZGYAHA8YSHJPM6D7503 (which fixed the 5 `tools::tests` terminal tests; these ~38 are a separate problem).

Failing areas (in `crates/claude-agent/src/`):
- `terminal_manager::tests` — `cargo test -p claude-agent --lib terminal_manager::tests` reports `28 passed; 21 failed`.
- `session::tests`, `path_validator::tests`, `capability_validation::tests`, and others — most failing with `Session("No storage path configured")` or `os error 22` storage-directory errors.
- `server::tests::test_json_rpc_error_response_format` — **hangs** (>60s, indefinitely).

Likely root cause: the test fixture/setup for the session-storage path regressed (tests build a session/agent without configuring a storage path), or a production contract changed (compare with how `tools::tests` was fixed in 01KRXK1Z — a `validate_terminal_capability` precondition was added without updating bare-fixture tests; a similar storage-path precondition may have been added). Investigate `git log`/`git blame` on the session-storage and `TerminalManager`/`Session` setup code to pin when and why it broke.

## Acceptance Criteria
- [ ] `cargo test -p claude-agent` passes — zero failures, including all `terminal_manager::tests`, `session::tests`, `path_validator::tests`, `capability_validation::tests`.
- [ ] `server::tests::test_json_rpc_error_response_format` completes (no hang) — fix the underlying wait, do not `#[ignore]` it.
- [ ] No assertion was weakened and no test was `#[ignore]`d to pass — the fix is a real fixture/setup or production-contract fix.

## Tests
- [ ] `cargo test -p claude-agent` — all green, with a hard per-test timeout to catch the hang regressing.
- [ ] `cargo clippy -p claude-agent --all-targets -- -D warnings` and `cargo build --workspace` — clean.

## Workflow
- This is a test/fixture repair (likely a shared setup helper for the session storage path, mirroring the `create_test_terminal_manager` helper that fixed 01KRXK1Z) — the existing `claude-agent` suite is the regression gate. Investigate the hang separately with a bounded-timeout run.

Not plugin-platform scope — standalone `claude-agent` tech debt. #test-failure