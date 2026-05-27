---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8780
title: 'Init: lockfile path must be scope-aware (don''t pollute cwd in user mode)'
---
## What

`sah init user` writes `mirdan-lock.json` into the **current working directory**, not the user's home. Both `SkillDeployment` and `AgentDeployment` call helpers (`apps/swissarmyhammer-cli/src/commands/skill.rs::deploy_all_skills` and `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs::load_agent_project_lockfile`) that resolve the lockfile root as `std::env::current_dir()` regardless of scope. `LockfileCleanup::deinit` (`components/mod.rs`) does the same in reverse. Meanwhile the skill/agent stores correctly target `~/.skills/` / `~/.agents/` for user scope via `mirdan::store::{skill_store_dir,agent_store_dir}(global=true)`. The lockfile and the store it tracks are now misaligned in user mode — and any user who runs `sah init user` from `~` ends up with a stray `~/mirdan-lock.json`.

Fix:

- Define a single helper `lockfile_root(scope: InitScope) -> PathBuf` (lives in mirdan, e.g. `mirdan::lockfile::root_for_scope` or `mirdan::store::lockfile_root_for_scope`). User → the same root that hosts the global store (e.g. `dirs::home_dir()`, matching where `~/.skills/mirdan-lock.json` and `~/.agents/` live). Project/Local → `std::env::current_dir()` (existing behavior).
- Replace every `std::env::current_dir()` lockfile call site in the CLI with this helper:
  - `apps/swissarmyhammer-cli/src/commands/skill.rs::deploy_all_skills` (its `project_root` declaration).
  - `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs::load_agent_project_lockfile`.
  - `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs::LockfileCleanup::deinit`.
- Plumb `scope` through `deploy_all_skills` / `init_all_agents` / `save_lockfile_and_report` so they can pass it to the helper (today they only thread `global: bool`; convert to `&InitScope` or extend the existing `global` flag with the resolved root).
- Verify by running the existing `sah init user` flow under an isolated HOME and asserting `~/.../mirdan-lock.json` is written and `<cwd>/mirdan-lock.json` is **not** created.

This is a real correctness bug introduced by the user-scope work — not cosmetic.

## Acceptance Criteria
- [x] `sah init user` writes the lockfile to the global location (in `~`), not the current working directory; `<cwd>/mirdan-lock.json` is not created.
- [x] `sah init` / `sah init local` still write `<git-root>/mirdan-lock.json` (no regression).
- [x] `sah deinit user` cleans up the global lockfile, not a cwd-relative one.
- [x] Exactly one helper resolves the lockfile root by scope; no `std::env::current_dir()` call remains in the lockfile flow.

## Tests
- [x] `#[serial_test::serial(home_env)]` test in `components/mod.rs` (or a dedicated lockfile test): `IsolatedTestEnvironment` + `CurrentDirGuard` into a temp non-HOME cwd; run the relevant component(s) with `InitScope::User`; assert `~/mirdan-lock.json` (or whatever the helper chooses) exists and the cwd has no `mirdan-lock.json`.
- [x] Add a mirdan unit test for `lockfile_root_for_scope(User)` / `(Project)` returning the expected paths.
- [x] `cargo test -p mirdan -p swissarmyhammer-cli` green.

## Workflow
- Use `/tdd` — write the cwd-pollution regression first. #init-doctor

## Implementation Notes

- Helper `mirdan::lockfile::lockfile_root_for_scope(scope: &InitScope) -> Result<PathBuf, String>` added in `crates/mirdan/src/lockfile.rs`. Returns `dirs::home_dir()` for `User`, `std::env::current_dir()` for `Project`/`Local`.
- `deploy_all_skills` and `load_project_lockfile` in `apps/swissarmyhammer-cli/src/commands/skill.rs` now take `&InitScope` and use the helper.
- `init_all_agents` and `load_agent_project_lockfile` in `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs` now take `&InitScope` and use the helper.
- `LockfileCleanup::deinit` uses the helper instead of `std::env::current_dir()`.
- Regression tests in `components/mod.rs`: `test_agent_deployment_user_scope_lockfile_in_home_not_cwd` and `test_lockfile_cleanup_user_scope_targets_home_not_cwd` both use `IsolatedTestEnvironment` + `CurrentDirGuard` to prove cwd is never polluted in user scope.
- Unit tests in `crates/mirdan/src/lockfile.rs`: `test_lockfile_root_for_scope_user` and `test_lockfile_root_for_scope_project` verify the helper directly.
- `cargo test -p mirdan -p swissarmyhammer-cli` green: 1434 tests, 0 failures. `cargo clippy -p mirdan -p swissarmyhammer-cli --all-targets -- -D warnings` clean.