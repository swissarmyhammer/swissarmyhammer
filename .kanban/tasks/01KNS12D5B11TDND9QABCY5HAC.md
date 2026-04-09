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

Model exactly on `shelltool-cli/src/registry.rs`.

`KanbanMcpRegistration` implements `Initializable` and registers/unregisters `kanban serve` in all detected agent MCP config files via `mirdan`.

```rust
pub struct KanbanMcpRegistration;

impl Initializable for KanbanMcpRegistration {
    fn name(&self) -> &str { "kanban-mcp-registration" }
    fn category(&self) -> &str { "configuration" }
    fn priority(&self) -> i32 { 10 }
    fn init(&self, scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        // mirdan::agents::load_agents_config() → get_detected_agents()
        // McpServerEntry { command: "kanban", args: ["serve"], env: {} }
        // mirdan::mcp_config::register_mcp_server(path, servers_key, "kanban", &entry)
    }
    fn deinit(...) -> Vec<InitResult> {
        // mirdan::mcp_config::unregister_mcp_server(path, servers_key, "kanban")
    }
}

pub fn register_all(registry: &mut InitRegistry) {
    registry.register(KanbanMcpRegistration);
}
```

Note: shelltool's `register_all` also registers `ShellExecuteTool` for skill deployment. Kanban has no equivalent skill-deployment component, so `register_all` only registers the MCP registration component.

## Acceptance Criteria
- [ ] `kanban init` registers `kanban` in detected agent MCP configs
- [ ] `kanban deinit` removes it idempotently
- [ ] `cargo check -p kanban-cli` passes

## Tests
- [ ] Unit test: `KanbanMcpRegistration::name()` returns `"kanban-mcp-registration"`
- [ ] Unit test: `KanbanMcpRegistration::priority()` returns `10`
- [ ] Unit test: `register_all` populates registry with exactly 1 component
- [ ] Unit test: `init` returns exactly 1 result
- [ ] Unit test: `deinit` returns exactly 1 result
- [ ] Test file: `kanban-cli/src/registry.rs` in `#[cfg(test)]` module

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.
