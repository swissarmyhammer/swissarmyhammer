---
assignees:
- claude-code
depends_on:
- 01KNS10MMDVZG731XKM390C682
position_column: todo
position_ordinal: ac80
project: kanban-mcp
title: 'kanban-cli: implement registry.rs — KanbanMcpRegistration for init/deinit'
---
## What

Create `kanban-cli/src/registry.rs` — the init/deinit component registry for kanban.

Two `Initializable` components, matching shelltool's pattern:

### Component 1: `KanbanMcpRegistration` (priority 10)

Registers/unregisters `kanban serve` as an MCP server in all detected agent configs via mirdan. Model on `ShelltoolMcpRegistration` in `shelltool-cli/src/registry.rs`.

```rust
pub struct KanbanMcpRegistration;

impl Initializable for KanbanMcpRegistration {
    fn name(&self) -> &str { "kanban-mcp-registration" }
    fn category(&self) -> &str { "configuration" }
    fn priority(&self) -> i32 { 10 }
    fn init(...) { /* mirdan::mcp_config::register_mcp_server */ }
    fn deinit(...) { /* mirdan::mcp_config::unregister_mcp_server */ }
}
```

### Component 2: `KanbanSkillDeployment` (priority 20)

Deploys/removes the builtin `kanban` skill to all detected agents. Model on `ShellExecuteTool`'s `Initializable` impl in `swissarmyhammer-tools/src/mcp/tools/shell/mod.rs`.

```rust
pub struct KanbanSkillDeployment;

impl Initializable for KanbanSkillDeployment {
    fn name(&self) -> &str { "kanban-skill-deployment" }
    fn category(&self) -> &str { "skills" }
    fn priority(&self) -> i32 { 20 }
    fn init(...) {
        // 1. swissarmyhammer_skills::SkillResolver::new().resolve_builtins().get("kanban")
        // 2. Render {{version}} in skill.instructions via swissarmyhammer_templating
        // 3. Write rendered SKILL.md to temp dir
        // 4. mirdan::install::deploy_skill_to_agents("kanban", &skill_dir, None, false)
    }
    fn deinit(...) {
        // mirdan::install::uninstall_skill("kanban", None, false)
    }
}
```

### Wire-up

```rust
pub fn register_all(registry: &mut InitRegistry) {
    registry.register(KanbanMcpRegistration);
    registry.register(KanbanSkillDeployment);
}
```

## Acceptance Criteria
- [ ] `kanban init` registers `kanban` MCP server in detected agent configs
- [ ] `kanban init` deploys the kanban skill to `.claude/skills/kanban/` (or agent equivalent)
- [ ] `kanban deinit` removes MCP registration idempotently
- [ ] `kanban deinit` removes the kanban skill from agents
- [ ] `cargo check -p kanban-cli` passes

## Tests
- [ ] Unit test: `KanbanMcpRegistration::name()` returns `"kanban-mcp-registration"`, priority 10
- [ ] Unit test: `KanbanSkillDeployment::name()` returns `"kanban-skill-deployment"`, priority 20
- [ ] Unit test: `register_all` populates registry with exactly 2 components
- [ ] Unit test: both `init` and `deinit` return at least 1 result each
- [ ] Test file: `kanban-cli/src/registry.rs` in `#[cfg(test)]` module

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.
