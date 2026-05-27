---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff9380
title: Isolate `commands::doctor` and `commands::model` CLI tests that leak `.sah/` into the crate cwd
---
## What

Follow-up discovered while implementing 01KSK2EAZRFG3V0PBMRFQS4H0G.

After fixing the three tests in that card (`test_mcp_registration_init_warns_when_no_agent_has_mcp_config`, `test_skill_deployment_init_returns_one_result`, `test_skill_deployment_deinit_returns_one_result`), running `cargo test -p swissarmyhammer-cli --lib` still leaves a `.sah/` directory (with `.gitignore` + empty `tmp/`) at `apps/swissarmyhammer-cli/`.

Bisected to two test groups outside the original card's scope:

1. **`commands::doctor` tests** (`apps/swissarmyhammer-cli/src/commands/doctor/...`) — running `cargo test -p swissarmyhammer-cli --lib commands::doctor` alone reproduces the `.sah/` skeleton. They appear to construct/initialize a SAH directory at the live cwd.
2. **`commands::model::use_command::tests::test_execute_use_command_builtin_agent*`** (lines ~222, ~254, ~268 of `apps/swissarmyhammer-cli/src/commands/model/use_command.rs`) — these call `execute_use_command` with a real builtin agent name in the live cwd, with no `CurrentDirGuard` / `IsolatedTestEnvironment`. `ModelManager::use_agent(.., &ModelPaths::sah())` then writes `.sah/sah.yaml` at cwd. The sibling test at line ~296 (`test_execute_use_command_with_temp_config`) already does the isolation; just mirror it onto the other call sites.

## Fix Applied

Wrapped offending tests in the canonical `IsolatedTestEnvironment + CurrentDirGuard + #[serial_test::serial(cwd)]` pattern from `commands::registry::tests::test_init_runs_without_panic`.

During implementation, discovered the leak was wider than the original bisect. Root cause: `CliContextBuilder::build_async()` calls `get_swissarmyhammer_dir()` (and `CliToolContext::new()` does similar) which creates `.sah/` at cwd via `SwissarmyhammerDirectory::from_custom_root().new()` (writes `.gitignore` + creates `tmp/` subdir). Any test that constructs `CliContext` / `CliToolContext` (directly or via `create_test_context()` helpers) without CWD isolation leaks.

**Files isolated:**
- `commands/doctor/mod.rs::test_run_diagnostics`
- `commands/model/use_command.rs` — all 6 tests that call `create_test_context()` (the existing `test_execute_use_command_with_temp_config` was also migrated from the homegrown `DirGuard` to the canonical `CurrentDirGuard`)
- `commands/prompt/list.rs` — all 10 async tests
- `commands/prompt/mod.rs::test_run_prompt_command_typed_list`, `test_run_prompt_command_typed_test_with_invalid_prompt`
- `commands/prompt/test.rs::test_execute_test_command_file_not_found`, `test_execute_test_command_missing_prompt_and_file`
- `commands/serve/mod.rs::test_handle_command_signature`
- `context.rs` — all 9 async tests that construct `CliContext`
- `mcp_integration.rs::test_cli_tool_context_creation`, `test_all_registered_tools_pass_cli_validation`, `test_create_arguments`, `test_isolated_tool_execution`
- `validate.rs::test_validate_all_handles_partial_templates`, `test_validate_tools_functionality`, `test_validate_all_with_tools_flag`, `test_validate_tools_error_handling`
- `cli_executor.rs::test_executor_creation`, `test_help_command` (had `IsolatedTestEnvironment` only, added `CurrentDirGuard`)

## Acceptance Criteria
- [x] `commands::doctor` tests no longer leave `.sah/` at the crate cwd.
- [x] `commands::model::use_command` tests that exercise real builtins are wrapped in the canonical isolation pattern.
- [x] After `rm -rf apps/swissarmyhammer-cli/{.skills,.claude,.github,.zed,.sah,.prompts,.agents,mirdan-lock.json,.mcp.json}` and `cargo test -p swissarmyhammer-cli --lib`, `git status -s apps/swissarmyhammer-cli/` shows zero new untracked entries.
- [x] `cargo clippy -p swissarmyhammer-cli --all-targets -- -D warnings` clean.

## Tests
- [x] All affected tests still pass (425/425 in `cargo test -p swissarmyhammer-cli --lib`).
- [x] Workspace stays clean after a full `cargo test -p swissarmyhammer-cli --lib` run from a clean tree.

## Workflow
- Small, focused fix mirroring 01KSK2EAZRFG3V0PBMRFQS4H0G. No `/tdd`. #test-isolation