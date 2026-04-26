---
position_column: done
position_ordinal: ffff9080
title: Create swissarmyhammer-commands crate with Command trait and CommandContext
---
Create the new `swissarmyhammer-commands` crate that depends on `swissarmyhammer-entity`.

## Scope

- Scaffold crate with `Cargo.toml`, `lib.rs`
- Define the `Command` trait with required `available(&self, ctx: &CommandContext) -> bool` and `async execute(&self, ctx: &CommandContext) -> Result<Value>` — no default impls, both must be implemented
- Define `CommandContext` struct: `command_id`, `scope_chain: Vec<String>`, `target: Option<String>`, `args: HashMap<String, Value>`, plus service access (`EntityCache`, `UIState`, `UndoStack`)
- Implement `CommandContext` helper methods: `resolve_moniker(entity_type)`, `has_in_scope(entity_type)`, `arg(name)`, `get_entity(type, id)`, `active_view()`, `can_undo()`, `can_redo()`
- Define `CommandDef` struct for YAML-loaded command metadata: `id`, `name`, `scope`, `visible`, `keys`, `params`, `undoable`, `context_menu`
- Define `CommandInvocation` struct: `cmd`, `scope_chain`, `target`, `args`
- Add crate to workspace `Cargo.toml`

## Testing

- Unit test: `CommandContext::resolve_moniker` finds nearest matching entity type in scope chain
- Unit test: `CommandContext::resolve_moniker` returns None when entity type not in chain
- Unit test: `CommandContext::has_in_scope` true/false cases
- Unit test: `CommandContext::arg` retrieves explicit args
- Test: `CommandDef` serde round-trip from YAML
- Test: `CommandInvocation` construction with all fields