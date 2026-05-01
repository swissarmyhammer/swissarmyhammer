---
position_column: done
position_ordinal: f780
title: Extract AgentDeployment as Initializable component
---
## What

Extract `install_agents_via_mirdan()` from monolithic `init.rs` into a standalone `AgentDeployment` struct implementing `Initializable`.

- `init`: Resolves builtin agents, renders Liquid templates, deploys to `.agents/` store via mirdan, creates coding agent symlinks, updates lockfile
- `deinit`: Removes deployed agents, cleans symlinks, updates lockfile
- Priority: 31 (right after skills)
- `is_applicable`: Always true (all scopes)

**Files:**
- NEW: `swissarmyhammer-cli/src/commands/install/components/agent_deployment.rs`
- EDIT: `swissarmyhammer-cli/src/commands/install/components/mod.rs`
- EDIT: `swissarmyhammer-cli/src/commands/install/init.rs` — remove `install_agents_via_mirdan()`, `format_agent_md()`

## Acceptance Criteria
- [ ] `AgentDeployment` implements `Initializable`
- [ ] `init()` deploys same agents as current `install_agents_via_mirdan()`
- [ ] Liquid template rendering preserved
- [ ] Lockfile entries written correctly
- [ ] `deinit()` cleans up agents and lockfile
- [ ] Old functions removed from `init.rs`

## Tests
- [ ] `cargo test -p swissarmyhammer-cli` passes
- [ ] Manual: `sah init` deploys agents to `.agents/`, lockfile updated