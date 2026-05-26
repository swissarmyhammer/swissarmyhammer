---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8180
title: 'Doctor: don''t hard-fail outside a git repo (user-mode install)'
---
## What

`sah doctor` fails entirely when not run inside a git repository, which is exactly the case for a user-mode install (running `sah doctor` from `~`). In `apps/swissarmyhammer-cli/src/commands/doctor/mod.rs`, `Doctor::run_diagnostics_without_output` calls `find_git_repository_root()` and, on `None`, pushes an **Error** check and **returns early** (`return Ok(ExitCode::Error.into())`) — so no other diagnostics run and the command reports failure.

Change the git-repository check from fatal to informational:
- When a git root **is** found: keep the existing Ok check and run the project-scoped checks (`check_swissarmyhammer_directory`, and the project CLAUDE.md check).
- When **no** git root is found: push a **Warning** (not Error), do NOT return early, and skip only the project-scoped checks. All scope-independent checks (installation, PATH, file permissions, LSP, AVP, tool health, Claude config) must still run.

Concretely, refactor so `git_root` becomes an `Option<PathBuf>` and the project-only checks are gated behind `if let Some(root) = &git_root`. Do not let a missing repo force a non-zero exit code on its own — exit code should be driven by the actual check statuses via `get_exit_code()`.

This card is purely about removing the hard dependency on a git repo. The new agent-agnostic project+user install-stack checks are a separate card; keep this change minimal and focused on the gating.

## Acceptance Criteria
- [x] Running doctor diagnostics in a non-git directory returns `Ok` and does NOT short-circuit; the result set includes all scope-independent checks plus a single Warning-level "Git Repository" check.
- [x] Running in a git repo behaves as before (Ok "Git Repository" check + `.sah` directory checks).
- [x] A missing git repo alone does not produce a non-zero exit code (no Error-status checks added for it).

## Tests
- [x] Add a `#[tokio::test]` (with `#[serial_test::serial(cwd)]`) in `doctor/mod.rs` using `IsolatedTestEnvironment` + `CurrentDirGuard` to run `run_diagnostics_without_output` in a temp non-git dir; assert it returns `Ok`, that there is a "Git Repository" check with `CheckStatus::Warning`, that no check has `CheckStatus::Error` solely due to the missing repo, and that other checks (e.g. installation method) are present.
- [x] Keep/adjust the existing `test_run_diagnostics` so it still passes (it runs in the repo and should see the Ok git check).
- [x] `cargo test -p swissarmyhammer-cli doctor` runs green.

## Workflow
- Use `/tdd` — write the non-git-dir test first (it should fail today because doctor errors out), then make the gating change. #init-doctor

## Implementation Notes
- `run_diagnostics_without_output`: `git_root` is now `Option<PathBuf>`; missing repo pushes a Warning and does not short-circuit. Project-scoped checks (`check_swissarmyhammer_directory` and project `CLAUDE.md`) are gated behind `if let Some(root) = &git_root`.
- Made `check_claude_md_at` public and moved the project `CLAUDE.md` check out of `run_configuration_checks` into the gated block; removed the now-redundant `check_claude_md` wrapper that re-called `find_git_repository_root()` internally. `run_configuration_checks` now only runs the scope-independent `check_claude_config`.
- Removed the now-unused `ExitCode` re-export from `doctor/types.rs`.
- `cargo test -p swissarmyhammer-cli doctor` and `cargo clippy -p swissarmyhammer-cli --all-targets` both green (zero warnings).