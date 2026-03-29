---
assignees:
- claude-code
position_column: doing
position_ordinal: '80'
title: Implement commands_for_scope in Rust with comprehensive tests
---
## What

Pure Rust function that takes a scope chain and returns all available commands with fully resolved names. This is the single source of truth for what commands are available and what they're called.

### Files to create/modify
- `kanban-app/src/commands.rs` — new `commands_for_scope()` function + `ResolvedCommand` struct

### Function signature
```rust
pub struct ResolvedCommand {
    pub id: String,
    pub name: String,
    pub target: Option<String>,
    pub context_menu: bool,
    pub keys: Option<KeysDef>,
    pub available: bool,
}

pub fn commands_for_scope(
    scope_chain: &[String],
    registry: &CommandsRegistry,
    command_impls: &HashMap<String, Arc<dyn Command>>,
    fields: Option<&FieldsContext>,
    ui_state: &UIState,
    context_menu_only: bool,
) -> Vec<ResolvedCommand>
```

### Logic
1. Walk scope chain innermost first. For each moniker type:id, look up entity schema via fields.get_entity(type), get its commands, resolve {{entity.type}}, set target = moniker
2. Add global commands (no scope requirement) from registry
3. Check available() on each via command_impls
4. Paste: resolve {{entity.type}} from clipboard_entity_type
5. Dedup by (id, target)
6. Filter context_menu_only if requested

### Tests (realistic scope chains)
- [ ] board scope: undo, redo available; no copy/cut
- [ ] column scope: paste task available when clipboard has task
- [ ] task scope: copy task, cut task, inspect, archive; paste tag when clipboard has tag
- [ ] tag-on-task scope: copy tag + copy task both present, cut tag + cut task, inspect tag + inspect task
- [ ] tag-on-task with tag clipboard: paste tag available
- [ ] tag-on-task with task clipboard: paste task NOT available (no column in innermost)
- [ ] all names fully resolved, no {{entity.type}} templates
- [ ] context_menu_only filters correctly
- [ ] empty scope: only global commands

## Acceptance Criteria
- [ ] Pure function, no Tauri dependency — fully testable
- [ ] All tests pass
- [ ] Returns both entity schema commands AND registry global commands
- [ ] Names fully resolved
- [ ] `cargo nextest run -p kanban-app` passes"
<parameter name="assignees">[]