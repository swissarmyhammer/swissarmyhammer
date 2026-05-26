---
assignees:
- claude-code
position_column: todo
position_ordinal: '8480'
title: Move path-safety + store-cleanup helpers to mirdan::store; delete dead MCP legacy fallback
---
## What

`apps/swissarmyhammer-cli/src/commands/install/components/mod.rs` has store-management helpers that duplicate or belong next to existing mirdan store code:

- `is_safe_name(name: &str) -> bool` — single-segment path-traversal sanitizer.
- `is_safe_relative_path(path: &str) -> bool` — multi-segment sibling.
- `remove_store_entries(...)` — removes named entries from a store dir and their symlinks across agent link dirs.
- `remove_single_store_entry(...)` — per-entry helper.
- `remove_if_symlink(path)` — safety-critical: only removes a path if it is a symlink.

These have no CLI-specific concerns; they are mirdan store mechanics. `mirdan::store` already owns `sanitize_dir_name`, `symlink_name`, `create_skill_link`, `remove_if_exists`, `store_entry_still_referenced`.

Also: `McpRegistration` (same file) still carries a legacy fallback path:
- `install_project_legacy(reporter)` and `uninstall_project_legacy(reporter)` — only triggered when **no** detected agent has an `mcp_config` populated. With the current `agents_default.yaml` that is unreachable for any real install: the fallback only fires when `installed_count == 0`, which means zero detected agents have an MCP config, in which case writing `.mcp.json` doesn't serve any agent anyway. The cleanup helper `cleanup_empty_mcp_servers` only exists for this path.

Move + delete:

- Move `is_safe_name`, `is_safe_relative_path` into `crates/mirdan/src/store.rs` (or a `mirdan::names` module) and re-export from `mirdan::store`. Update all call sites in `apps/swissarmyhammer-cli/src/commands/{install/components/mod.rs,skill.rs}`.
- Move `remove_store_entries`, `remove_single_store_entry`, `remove_if_symlink` into `mirdan::store`. Update call sites in `SkillDeployment::deinit`, `AgentDeployment::deinit`, and the components helper.
- Delete `install_project_legacy`, `uninstall_project_legacy`, `cleanup_empty_mcp_servers` and the `installed_count == 0` fallback branches in `McpRegistration::init`/`deinit`. The component's success path already iterates detected agents via mirdan; no fallback is needed once mirdan handles every supported agent.
- Keep all existing tests; relocate the helper unit tests to mirdan's test module.

## Acceptance Criteria
- [ ] No `is_safe_*` / `remove_store_*` / `remove_if_symlink` definitions remain in `apps/swissarmyhammer-cli/`; the CLI consumes them via `mirdan::store::…`.
- [ ] `install_project_legacy` / `uninstall_project_legacy` / `cleanup_empty_mcp_servers` are gone; `McpRegistration` reports zero-installed agents as a Warning (with a fix hint) instead of writing a non-agent-bound `.mcp.json`.
- [ ] `cargo build -p mirdan -p swissarmyhammer-cli` green; clippy clean with `-D warnings`.
- [ ] All previously-passing install/skill/agent tests still pass.

## Tests
- [ ] Move the existing `test_is_safe_name` / `test_is_safe_relative_path` unit tests to `mirdan::store`'s test module.
- [ ] Add a `McpRegistration::init` test asserting the no-detected-agents path emits a Warning result rather than touching `.mcp.json` (replacing the deleted legacy behavior).
- [ ] `cargo test -p mirdan -p swissarmyhammer-cli` green.

## Workflow
- Use `/tdd` — write the moved-helpers tests in mirdan first, then move and delete. #init-doctor