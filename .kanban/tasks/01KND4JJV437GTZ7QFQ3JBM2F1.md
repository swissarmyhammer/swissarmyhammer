---
assignees:
- claude-code
depends_on:
- 01KND4J0P5NK9XY1VM3ZCQ1BY9
position_column: done
position_ordinal: ffffffffffffffffffff8780
title: 'VT-2: VirtualTagStrategy trait and registry'
---
## What

Create the strategy abstraction for virtual tags. Each virtual tag has a strategy that determines whether it applies to a given task, plus metadata (color, description) and **commands** that appear in the context menu when the user right-clicks the virtual tag pill.

**Files to create/modify:**
- `swissarmyhammer-kanban/src/virtual_tags.rs` — new module with:

```rust
/// A command declared by a virtual tag strategy.
/// Mirrors the EntityCommand structure used by entity YAML schemas.
pub struct VirtualTagCommand {
    pub id: String,
    pub name: String,
    pub context_menu: bool,
    pub keys: Option<KeyBindings>,
}

pub trait VirtualTagStrategy: Send + Sync {
    /// The tag slug (e.g. "READY", "BLOCKED").
    fn slug(&self) -> &str;
    /// Display color (6-char hex).
    fn color(&self) -> &str;
    /// Human-readable description.
    fn description(&self) -> &str;
    /// Commands available on this virtual tag's context menu.
    fn commands(&self) -> Vec<VirtualTagCommand>;
    /// Whether this virtual tag applies to the given task.
    fn matches(&self, entity: &Entity, all_tasks: &[Entity], terminal_column_id: &str) -> bool;
}
```

- `VirtualTagRegistry` struct: maps tag slug → `Box<dyn VirtualTagStrategy>`
  - `register()`, `get()`, `all()` for CRUD
  - `evaluate(entity, all_tasks, terminal_column_id) -> Vec<String>` — returns matching slugs
  - `metadata() -> Vec<VirtualTagMeta>` — returns all slugs with color/description/commands for API serialization
  - `is_virtual_slug(slug) -> bool` — quick check for guards in tag/untag commands
- `fn default_virtual_tag_registry() -> VirtualTagRegistry` — returns empty registry (strategies registered in later cards)
- `swissarmyhammer-kanban/src/lib.rs` — add `pub mod virtual_tags;`

The `commands()` method follows the same pattern as view commands in YAML — declarative command definitions that the frontend wires to execute handlers. The execute handlers for virtual tag commands dispatch to the backend like any other command (via `backendDispatch`), where kanban command handlers implement the actual logic.

## Acceptance Criteria
- [ ] `VirtualTagStrategy` trait defined with `matches()`, `slug()`, `color()`, `description()`, `commands()` methods
- [ ] `VirtualTagCommand` struct mirrors entity command shape (id, name, context_menu, keys)
- [ ] `VirtualTagRegistry` with `register()`, `get()`, `all()`, `evaluate()`, `metadata()`, `is_virtual_slug()` methods
- [ ] `default_virtual_tag_registry()` returns a registry (initially empty)
- [ ] Module compiles and is reachable from crate root

## Tests
- [ ] `swissarmyhammer-kanban/src/virtual_tags.rs` — unit tests for registry CRUD, evaluate with mock strategy, metadata serialization
- [ ] `swissarmyhammer-kanban/src/virtual_tags.rs` — unit test: mock strategy with commands, verify commands included in metadata
- [ ] `swissarmyhammer-kanban/src/virtual_tags.rs` — unit test: `is_virtual_slug` returns true/false correctly
- [ ] `cargo nextest run -p swissarmyhammer-kanban` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

#virtual-tags