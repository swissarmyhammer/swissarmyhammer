---
assignees:
- claude-code
depends_on:
- 01KNS10MMDVZG731XKM390C682
- 01KP0V8285PSG6GSMCVRXG8VRJ
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffb280
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
- [x] `kanban-cli/src/commands/registry.rs` exists
- [x] `KanbanMcpRegistration` handles MCP server registration only
- [x] `register_all` registers MCP registration + skill deployment (from commands/skill.rs)
- [x] `cargo check -p kanban-cli` passes

## Tests
- [x] Unit test: `KanbanMcpRegistration::name()` returns `"kanban-mcp-registration"`, priority 10
- [x] Unit test: `register_all` populates registry with exactly 2 components