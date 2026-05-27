---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffff180
title: 'Test isolation bug: skill/agent/workspace deployment pollutes the real source tree'
---
## What
One or more tests deploy skills / agents / workspace files into the **real repository source tree** instead of an isolated temp dir. Observed pollution (all untracked, now cleaned up manually):

- Repo-root directories `swissarmyhammer-cli/` and `shelltool-cli/` — full mirdan deployments: `.claude/skills`, `.claude/agents`, `.zed/skills`, `.github/copilot/skills`, `.agents/*/AGENT.md`, `.skills/*/SKILL.md`, `.sah/sah.yaml` (`model: GLM-5.0`), `.sah/mcp.log`, `.shell/mcp.log`, `mirdan-lock.json`.
- `apps/swissarmyhammer-cli/.{claude,github,skills,zed}` and `apps/shelltool-cli/.{claude,github,skills,zed}` — same kind of deployment landing in the crate manifest dirs.
- Tracked files overwritten in place: `apps/code-context-cli/.skills/code-context/SKILL.md`, `apps/code-context-cli/.skills/lsp/SKILL.md`; `apps/shelltool-cli/.mcp.json` emptied to `{"mcpServers":{}}` (reverted via `git checkout`).

## Acceptance Criteria
- [x] The test(s) that deploy skills/agents/workspace/mirdan files without temp-dir isolation are identified and fixed to use an isolated temp dir (or a `CurrentDirGuard` scoped to a temp dir).
- [x] Running the affected crates' test suites leaves `git status` clean — no `.claude/.github/.skills/.zed/.sah/.shell` dirs, no `mirdan-lock.json`, no root crate-named dirs, no modified tracked `SKILL.md` / `.mcp.json` files.
- [x] If deployment code silently defaults to `current_dir()` in a way tests can trigger against the repo, that is hardened or the test is given an explicit root.

## Tests
- [x] After the fix, run the affected crates' tests and assert (in the test or by inspection) that deployment targets a temp dir.
- [x] `cargo test` for the affected crate(s) is green and the working tree stays clean. #bug

## Implementation Notes

### Root cause
mirdan's deployment functions resolve their targets from **process-relative paths** (skill stores `.skills`, per-agent `.claude/skills`/`.zed/skills`/`.github/copilot/skills`, MCP `.mcp.json`, lockfiles). This is correct for production (a CLI is rooted at the user's CWD). The bug is that several unit tests called this real deployment code with CWD = the crate manifest dir.

### UNIFIED SERIAL GROUP (re-open fix, 2026-05-17 — supersedes earlier serialization notes)
**Unified group name per crate: `cwd` (a named `serial_test` group) in all four crates.**

The earlier pass used FOUR independent mutexes — `#[serial(env)]`, bare `#[serial]`, `#[serial(cwd)]`, and `CurrentDirGuard`'s internal `CURRENT_DIR_LOCK` — which do NOT serialize against each other. Tests under different groups raced on the process-global CWD, so a deinit could run with CWD restored to the crate dir and strip the tracked `.mcp.json`. Every CWD-touching test (mutating OR reading) in each crate is now on the single `cwd` group. Tests touching CWD *and* an env var carry `#[serial(cwd, env)]`.

Full list of tests moved onto the `cwd` group:

**shelltool-cli** (`cwd`):
- `commands/skill.rs`: `test_deploy_shell_skill_returns_valid_result`, `test_shelltool_skill_deployment_init`, `test_shelltool_skill_deployment_deinit` (were bare `#[serial]`).
- `logging.rs`: `init_tracing_creates_mcp_log_under_shell_dir` (was bare `#[serial]`).
- `main.rs`: `dispatch_command_init_local_runs_registry`, `dispatch_command_deinit_local_runs_registry` (had NO serial attribute).
- `commands/doctor.rs`: `test_check_git_repository_not_in_git` (was `#[serial(env)]` → now `#[serial(cwd)]`), `test_run_diagnostics`, `test_run_doctor`, `test_run_doctor_verbose`, `test_check_git_repository` (had no serial attribute; read CWD via `find_git_repository_root`).
- `commands/registry.rs`: `test_init_and_deinit_register_success_path` (was `#[serial(env)]` → now `#[serial(cwd, env)]`); `test_init_returns_ok_result`, `test_deinit_returns_ok_result`, `test_init_global_scope`, `test_deinit_global_scope` (had NO isolation at all — these were the `.mcp.json` offenders; now use new `isolated_init_env()` = `IsolatedTestEnvironment` + `CurrentDirGuard` + `#[serial(cwd)]`).

**kanban-cli** (`cwd`):
- `commands/skill.rs`: `test_deploy_kanban_skill_returns_valid_result`, `test_kanban_skill_deployment_init`, `test_kanban_skill_deployment_deinit` (were bare `#[serial]`).
- `logging.rs`: `init_tracing_creates_mcp_log_when_kanban_dir_exists` (was bare `#[serial]`).
- `commands/doctor.rs`: `check_board_initialized_recognizes_entity_layout`, `check_board_initialized_warns_when_no_kanban_dir` (were bare `#[serial]`); `check_git_repository_produces_one_check`, `check_board_initialized_produces_one_check`, `run_diagnostics_runs_all_three_checks`, `run_doctor_non_verbose_returns_valid_exit_code`, `run_doctor_verbose_returns_valid_exit_code` (had no serial; read CWD).
- `commands/serve.rs`: `kanban_server_call_tool_resolves_cwd_for_init_board` (had NO serial attribute).
- `commands/registry.rs`: `test_init_returns_single_result`, `test_deinit_returns_single_result` (had NO isolation — `.mcp.json` offenders; now use new `isolated_init_env()` + `#[serial(cwd)]`).

**code-context-cli** (`cwd`):
- `commands/ops.rs`: `test_run_operation_get_status`, `test_run_operation_get_status_json` (already `#[serial(cwd)]` — unchanged).
- `commands/skill.rs`: `test_run_skill_returns_valid_exit_code` (was bare `#[serial]`).
- `logging.rs`: `init_tracing_creates_mcp_log_under_code_context_dir` (was bare `#[serial]`).
- `commands/doctor.rs`: `test_run_diagnostics`, `test_run_doctor`, `test_run_doctor_verbose`, `test_check_git_repository`, `test_check_index_directory`, `test_check_lsp_status` (had no serial; read CWD).
- `commands/registry.rs`: `test_init_returns_ok_result`, `test_deinit_returns_ok_result` (had NO isolation — `.mcp.json` offenders; now use new `isolated_init_env()` + `#[serial(cwd)]`).

**swissarmyhammer-cli** (`cwd`):
- `commands/skill.rs`, `commands/registry.rs`, `commands/model/use_command.rs`, `commands/doctor/checks.rs` — already on `#[serial(cwd)]` (unchanged).
- `mcp_integration.rs`: `test_cli_tool_context_creation`, `test_all_registered_tools_pass_cli_validation` (were bare `#[serial]`; `CliToolContext::new()` reads CWD).
- `commands/doctor/checks.rs`: `test_lsp_servers_check` (had NO serial — this was the actual flake-failure: `check_lsp_servers` reads CWD + runs project detection; a concurrent empty-tempdir test made the `rust-analyzer` assertion fail).
- `commands/doctor/mod.rs`: `test_run_diagnostics` (had no serial; reads CWD via `find_git_repository_root` + `check_lsp_servers`).
- `commands/install/settings.rs`: `test_project_key_returns_nonempty` (had no serial; `project_key()` falls back to `current_dir()`).
- `list.rs` `#[serial]` tests left as-is — they guard the `colored` terminal-override global, not CWD; correctly a separate unnamed group.

### The `.mcp.json` offenders
The committed `apps/{shelltool,kanban,code-context}-cli/.mcp.json` files were corrupted because each crate's `registry.rs` had `test_init_returns_ok_result` / `test_deinit_returns_ok_result` (`InitScope::Project`) calling `ShelltoolMcpRegistration`/`KanbanMcpRegistration`/`CodeContextMcpRegistration` `init`/`deinit` with **zero CWD isolation**. `init`/`deinit` resolve each detected agent's MCP config from a CWD-relative path (`.mcp.json`), so `deinit` stripped the server entry from the tracked file. Each is now wrapped in a new `isolated_init_env()` helper (`IsolatedTestEnvironment` for HOME + `CurrentDirGuard` for CWD) and carries `#[serial(cwd)]`.

### Files changed (re-open)
- `apps/shelltool-cli/src/commands/{skill,doctor,registry}.rs`, `apps/shelltool-cli/src/{logging,main}.rs`
- `apps/kanban-cli/src/commands/{skill,doctor,serve,registry}.rs`, `apps/kanban-cli/src/logging.rs`
- `apps/code-context-cli/src/commands/{skill,doctor,registry}.rs`, `apps/code-context-cli/src/logging.rs`
- `apps/swissarmyhammer-cli/src/commands/skill.rs`, `apps/swissarmyhammer-cli/src/mcp_integration.rs`, `apps/swissarmyhammer-cli/src/commands/doctor/{checks,mod}.rs`, `apps/swissarmyhammer-cli/src/commands/install/settings.rs`, `apps/swissarmyhammer-cli/tests/integration/{model_commands,model_performance_edge_casess}.rs` (model integration fixes from the earlier pass)

No `Cargo.toml` changes — `serial_test` already a dev-dependency in all four crates. No `#[ignore]`, no deleted assertions, no worktrees. `apps/kanban-app/*`, `ARCHITECTURE.md`, `Cargo.lock` untouched.

### Verification (re-open)
All four crates: `cargo clippy --tests -- -D warnings` clean (exit 0). Full test suites of `shelltool-cli`, `kanban-cli`, `code-context-cli`, `swissarmyhammer-cli` run **3 times with default parallelism** — all green every run (code-context 106+1+2, kanban 85+3+10+6, shelltool 46+4, swissarmyhammer 451+435+1+230+1+0+16+0 + 1 doc-test). After every run `git status --porcelain` diffed identical to the pre-run baseline — zero pollution, and `apps/{shelltool,kanban,code-context}-cli/.mcp.json` explicitly confirmed byte-unmodified each time. Broad pollution scan (`mirdan-lock.json`, `.claude/.github/.skills/.zed/.sah/.shell` dirs, `SKILL.md`, `AGENT.md`, root crate-named dirs) — zero matches all three runs.

A pre-existing, unrelated parallel-execution flake in the model integration tests (unsynchronized `HOME` env mutation) was filed as a separate task (`01KRTYAJDRZ48ECQFF1BR48RBP`); out of scope.

## Review Findings (2026-05-17 14:32)

### Warnings
- [x] `apps/swissarmyhammer-cli/src/commands/skill.rs` — skill-deployment tests carried no `#[serial(cwd)]`. FIXED: added `#[serial_test::serial(cwd)]`.
- [x] `apps/shelltool-cli/src/commands/skill.rs` — skill tests lacked `#[serial]`. FIXED: now on the unified `cwd` group.
- [x] `apps/kanban-cli/src/commands/skill.rs` — skill tests lacked `#[serial]`. FIXED: now on the unified `cwd` group.
- [x] `apps/code-context-cli/src/commands/skill.rs` — `test_run_skill_returns_valid_exit_code` lacked `#[serial]`. FIXED: now on the unified `cwd` group.

### Nits
- [x] Positive deployment-artifact assertions — skipped as permitted (invasive); isolation verified by the `git status` pollution check.
- [x] Verification re-run with the default parallel runner — done (3x, see Verification above).

## Re-open Findings (2026-05-17, this pass)
- [x] `.mcp.json` still corrupted after the earlier pass — root cause was the unisolated `registry.rs::test_init/deinit` tests in shelltool/kanban/code-context-cli (NOT a serial-group gap). FIXED: wrapped in `isolated_init_env()`.
- [x] Four independent serial mutexes (`serial(env)`, bare `serial`, `serial(cwd)`, `CURRENT_DIR_LOCK`) — FIXED: unified every CWD-touching test in each crate onto a single `cwd` group.
- [x] `swissarmyhammer-cli` `test_lsp_servers_check` flake-failed under parallelism (CWD-reading test not on the `cwd` group). FIXED: added `#[serial(cwd)]`; audited and fixed all sibling CWD-reading doctor/registry/mcp tests across all four crates.