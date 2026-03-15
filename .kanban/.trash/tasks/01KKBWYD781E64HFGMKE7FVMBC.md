---
position_column: todo
position_ordinal: a2
title: Extract monolithic init into Initializable components
---
## What

Break the monolithic `init.rs::install()` function into separate `Initializable` implementations. Each logical init step becomes its own struct implementing `Initializable`. These are NOT tools — they're system-level components.

**Components to extract:**

| Component | Priority | What it does |
|-----------|----------|-------------|
| `McpRegistration` | 10 | Registers `sah serve` in all agent configs (mirdan) |
| `ClaudeLocalScope` | 11 | Writes to `~/.claude.json` for Local target |
| `ProjectStructure` | 20 | Creates `.swissarmyhammer/`, `.prompts/`, `workflows/` |
| `SkillDeployment` | 30 | Renders + deploys builtin skills via mirdan |
| `AgentDeployment` | 31 | Renders + deploys builtin agents via mirdan |

Each component owns exactly one concern. The `BashDenial` step moves to card 4 (shell tool owns it).

**Files:**
- NEW: `swissarmyhammer-cli/src/commands/install/components/mod.rs`
- NEW: `swissarmyhammer-cli/src/commands/install/components/mcp_registration.rs`
- NEW: `swissarmyhammer-cli/src/commands/install/components/project_structure.rs`
- NEW: `swissarmyhammer-cli/src/commands/install/components/skill_deployment.rs`
- NEW: `swissarmyhammer-cli/src/commands/install/components/agent_deployment.rs`
- EDIT: `swissarmyhammer-cli/src/commands/install/init.rs` — replace hardcoded steps with `InitRegistry` iteration
- EDIT: `swissarmyhammer-cli/src/commands/install/deinit.rs` — same, call `deinit()` on each

The init function becomes:
```rust
pub fn install(target: InstallTarget) -> Result<(), String> {
    let scope = target.into(); // InstallTarget -> InitScope
    let mut registry = InitRegistry::new();
    register_system_components(&mut registry);
    // tools will be registered separately (card 4)
    let results = registry.run_all_init(&scope);
    display_results(&results);
    Ok(())
}
```

## Acceptance Criteria
- [ ] Each init step is its own struct implementing `Initializable`
- [ ] `init.rs::install()` uses `InitRegistry` instead of hardcoded function calls
- [ ] `deinit.rs::uninstall()` uses `InitRegistry::run_all_deinit()`
- [ ] Priority ordering is respected (project structure before skill deployment)
- [ ] `is_applicable()` returns false for scope-inappropriate components (e.g., `ClaudeLocalScope` only for Local)
- [ ] `InstallTarget` maps cleanly to `InitScope`
- [ ] Behavior is identical to current — same files written, same output

## Tests
- [ ] `cargo test -p swissarmyhammer-cli` passes
- [ ] Manual: `sah init` produces same config files as before
- [ ] Manual: `sah deinit` cleans up same files as before