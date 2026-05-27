---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff9280
title: Isolate three CLI install tests that write `.skills/`/`.claude/`/etc. into the workspace cwd
---
## What

Running `cargo test -p swissarmyhammer-cli --lib` leaves stray directories under `apps/swissarmyhammer-cli/`:
- `.skills/` (populated with deployed builtin SKILL.md files)
- `.claude/`, `.github/`, `.zed/` (empty parent dirs left behind by `deploy_agent_to_agents` iterating detected agents)
- `.sah/` (from `ProjectStructure::init` triggered by full-registry tests, even though those *do* isolate cwd — re-check)

Root cause: three tests call install components with `InitScope::Project` but **without** the `IsolatedTestEnvironment + CurrentDirGuard + #[serial_test::serial(cwd)]` pattern that the sibling tests in `apps/swissarmyhammer-cli/src/commands/registry.rs` already use:

1. `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs::tests::test_mcp_registration_init_warns_when_no_agent_has_mcp_config` (line ~1675) — calls `McpRegistration.init(&InitScope::Project, ...)`, writes/inspects `.mcp.json` at cwd. *This one already uses `IsolatedTestEnvironment` + `EnvGuard` for the MIRDAN_AGENTS_CONFIG env var, but no `CurrentDirGuard`*, so it still writes `.mcp.json` into the crate dir even though it asserts on the file's content via the env-pointed config.
2. `apps/swissarmyhammer-cli/src/commands/skill.rs::tests::test_skill_deployment_init_returns_one_result` (line ~620) — calls `SkillDeployment.init(&InitScope::Project, ...)`, which writes the whole `.skills/` tree and iterates detected agents creating sibling parent dirs.
3. `apps/swissarmyhammer-cli/src/commands/skill.rs::tests::test_skill_deployment_deinit_returns_one_result` (line ~630) — same issue from the deinit side; reads/removes from cwd.

Fix: wrap each test in the canonical isolation pattern:

```rust
#[test]
#[serial_test::serial(cwd)]
fn test_…() {
    let env = swissarmyhammer_common::test_utils::IsolatedTestEnvironment::new().expect("isolated env");
    let _cwd = swissarmyhammer_common::test_utils::CurrentDirGuard::new(env.temp_dir()).expect("cwd guard");
    // … existing body unchanged …
}
```

That's the same pattern used by `commands::registry::tests::{test_init_runs_without_panic, test_deinit_runs_without_panic, test_register_all_includes_skill_deployment}`. Reuse the same imports.

For test #1 (MCP), keep the existing `IsolatedTestEnvironment` (and the env-var guard) and add the `CurrentDirGuard` + `serial(cwd)` on top — don't double-isolate HOME.

After the fix: `cargo test -p swissarmyhammer-cli --lib` should leave the crate dir clean. Verify by `rm -rf apps/swissarmyhammer-cli/{.claude,.github,.skills,.zed,.sah,.prompts,.agents,mirdan-lock.json,.mcp.json}` (clean), running the suite, and checking `git status` — no untracked dirs should reappear.

## Acceptance Criteria
- [x] All three offenders use `IsolatedTestEnvironment + CurrentDirGuard + #[serial_test::serial(cwd)]`.
- [x] After running the three target tests, no `.skills/`, `.claude/`, `.github/`, `.zed/`, `.sah/`, `.prompts/`, `.agents/`, `mirdan-lock.json`, or `.mcp.json` appears under `apps/swissarmyhammer-cli/`.
- [x] All three tests still pass.
- [x] `cargo clippy -p swissarmyhammer-cli --all-targets -- -D warnings` clean.

## Tests
- [x] The three modified tests pass.
- [x] After the three tests run in isolation, the crate dir stays clean.

## Workflow
- Mirror the pattern from `commands::registry::tests`. Small, focused fix; no `/tdd`. #test-isolation

## Implementation Notes

Re-check finding from the task author's note about `.sah/`: it is **not** from `ProjectStructure::init` via full-registry tests. The registry tests (`commands::registry::tests::*`) are already isolated and stay clean. The remaining `.sah/` skeleton observed on full `cargo test -p swissarmyhammer-cli --lib` runs is created by two unrelated test groups outside this task's scope:

- `commands::doctor` tests
- `commands::model::use_command::tests::test_execute_use_command_builtin_agent*` (multiple sibling tests of the already-isolated `test_execute_use_command_with_temp_config`)

Filed as follow-up: **01KSK2TGFVYFV0QC7EXPWAX9DV** — "Isolate `commands::doctor` and `commands::model` CLI tests that leak `.sah/` into the crate cwd".

### Files changed
- `apps/swissarmyhammer-cli/src/commands/skill.rs`: added `CurrentDirGuard` + `IsolatedTestEnvironment` imports to the `tests` module; wrapped both `test_skill_deployment_init_returns_one_result` and `test_skill_deployment_deinit_returns_one_result` in the canonical isolation pattern with `#[serial_test::serial(cwd)]`.
- `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs`: in `test_mcp_registration_init_warns_when_no_agent_has_mcp_config`, expanded the existing `#[serial_test::serial(home_env)]` to `serial(home_env, cwd)`, added `CurrentDirGuard` to the existing `swissarmyhammer_common::test_utils` import, and pinned cwd via `CurrentDirGuard::new(env.temp_dir())`. HOME is not double-isolated.

### Verification
- `cargo test -p swissarmyhammer-cli --lib -- test_skill_deployment_init_returns_one_result test_skill_deployment_deinit_returns_one_result test_mcp_registration_init_warns_when_no_agent_has_mcp_config` → 3/3 pass, no stray dirs at crate cwd.
- Full `cargo test -p swissarmyhammer-cli --lib` → 425/425 pass.
- `cargo clippy -p swissarmyhammer-cli --all-targets -- -D warnings` → clean.