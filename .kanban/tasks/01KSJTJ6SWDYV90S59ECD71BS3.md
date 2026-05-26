---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff9080
title: 'Fix CI: `check_claude_config` should return Warning when claude binary is absent or fails'
---
## What

CI is failing on `apps/swissarmyhammer-cli/src/commands/doctor/tests::test_run_diagnostics_outside_git_repo` (both lib and bin test binaries) because the test asserts no `CheckStatus::Error` exists in the diagnostics result, but `check_claude_config` in `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs` returns `CheckStatus::Error` when the `claude` binary isn't on PATH. CI runners don't have claude installed, so the Error fires and the assertion panics.

Fix: downgrade the "claude binary not on PATH" / "claude binary failed to execute" / "claude mcp list returned non-zero" outcomes from `CheckStatus::Error` to `CheckStatus::Warning`. Claude not being on PATH is **optional infrastructure** — consistent with:
- `check_avp_in_path` (avp missing → Warning)
- `check_single_lsp_server` (LSP server missing → Warning)
- `check_claude_mcp_list`'s own "swissarmyhammer not configured" path (already Warning)

Doctor should still produce a useful report when claude isn't installed; it's not fatal. Only genuine misconfiguration (which doesn't exist in this code path) deserves Error.

Concrete changes to `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs`:
- `check_claude_config` — when `claude_path.is_none()`, push `CheckStatus::Warning` with the same fix hint. Keep the message text useful.
- `check_claude_mcp_list` — both the "failed to execute" branch and the "failed to run `claude mcp list`" branch downgrade `Error` → `Warning`. Keep the install/check hint.

The "swissarmyhammer not found in mcp list" branch is already Warning — leave it.

## Acceptance Criteria
- [ ] `check_claude_config` returns only `Ok` or `Warning` (never `Error`) regardless of claude binary presence or runtime behavior.
- [ ] `cargo test -p swissarmyhammer-cli commands::doctor::tests::test_run_diagnostics_outside_git_repo` passes both lib and bin test binaries.
- [ ] `cargo clippy -p swissarmyhammer-cli --all-targets -- -D warnings` clean.

## Tests
- [ ] Add a focused regression test in `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs` that mocks/forces the "claude not on PATH" case (e.g. by setting `PATH=""` under `#[serial_test::serial]` with proper RAII) and asserts the resulting check has `CheckStatus::Warning`, not `Error`.
- [ ] Existing `test_run_diagnostics_outside_git_repo` continues to pass and now does so even when `claude` is absent from PATH.
- [ ] Full `cargo test -p swissarmyhammer-cli` green.

## Workflow
- Small, principled fix; no `/tdd` ceremony — read the existing pattern in `checks.rs`, mirror it. #ci-fix