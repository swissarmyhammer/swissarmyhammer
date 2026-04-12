---
assignees:
- claude-code
depends_on:
- 01KNS10MMDVZG731XKM390C682
- 01KP0V8285PSG6GSMCVRXG8VRJ
position_column: todo
position_ordinal: ac80
project: kanban-mcp
title: 'kanban-cli: implement commands/registry.rs — KanbanMcpRegistration for init/deinit'
---
## What

Create `kanban-cli/src/commands/registry.rs` — the init/deinit component registry. MCP registration only; skill deployment lives in `commands/skill.rs`.

```rust
use crate::commands::skill::KanbanSkillDeployment;

pub fn register_all(registry: &mut InitRegistry) {
    registry.register(KanbanMcpRegistration);
    registry.register(KanbanSkillDeployment);
}
```

## Acceptance Criteria
- [ ] `kanban-cli/src/commands/registry.rs` exists
- [ ] `KanbanMcpRegistration` handles MCP server registration only
- [ ] `register_all` registers MCP registration + skill deployment (from commands/skill.rs)
- [ ] `cargo check -p kanban-cli` passes

## Tests
- [ ] Unit test: `KanbanMcpRegistration::name()` returns `"kanban-mcp-registration"`, priority 10
- [ ] Unit test: `register_all` populates registry with exactly 2 components
