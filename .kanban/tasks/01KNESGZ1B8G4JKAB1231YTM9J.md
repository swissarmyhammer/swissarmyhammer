---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffe80
title: 'VT-0: Entity-level predicate filtering with EntityFilterContext'
---
## What

Add predicate-based filtering to the entity layer (`swissarmyhammer-entity`) so that consumers like kanban don't have to implement ad-hoc `.filter()` closures over raw entity lists. The predicate receives an `EntityFilterContext` that provides global state beyond the single entity under evaluation.

### EntityFilterContext

```rust
pub struct EntityFilterContext<'a> {
    /// All entities of the type being filtered (the unfiltered superset).
    pub entities: &'a [Entity],
    /// Typed extras — domain-specific state injected by the caller.
    extras: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl<'a> EntityFilterContext<'a> {
    pub fn new(entities: &'a [Entity]) -> Self;

    /// Insert a typed value. Caller's domain types go here.
    pub fn insert<T: 'static + Send + Sync>(&mut self, value: T);

    /// Retrieve a typed value by type.
    pub fn get<T: 'static>(&self) -> Option<&T>;
}
```

### EntityContext::list_where

```rust
impl EntityContext {
    /// List entities of a type, filtered by a predicate with access to context.
    ///
    /// Loads all entities first (with computed fields derived), builds the
    /// context, then filters. Returns only entities where predicate returns true.
    pub async fn list_where<F>(
        &self,
        entity_type: &str,
        build_ctx: impl FnOnce(&[Entity]) -> EntityFilterContext<'_>,
        predicate: F,
    ) -> Result<Vec<Entity>>
    where
        F: Fn(&Entity, &EntityFilterContext) -> bool;
}
```

The `build_ctx` callback lets the caller populate the context after entities are loaded but before filtering — this is where kanban injects its VirtualTagRegistry, terminal column ID, etc. The entity layer never knows about those types.

**Files to create/modify:**
- `swissarmyhammer-entity/src/filter.rs` — new module with `EntityFilterContext`
- `swissarmyhammer-entity/src/context.rs` — add `list_where` method to `EntityContext`
- `swissarmyhammer-entity/src/lib.rs` — export `EntityFilterContext`

### Migration path
This card does NOT change existing kanban filtering. VT-4 will migrate `list tasks --tag` and `next task --tag` to use `list_where`. The old inline filters continue to work.

## Acceptance Criteria
- [ ] `EntityFilterContext` struct with `new()`, `insert<T>()`, `get<T>()` methods
- [ ] `EntityContext::list_where()` loads entities, builds context, applies predicate
- [ ] Computed fields are derived before filtering (existing behavior preserved)
- [ ] Context's `get<T>()` returns `None` for missing types (not panic)
- [ ] Works with closures that capture external state

## Tests
- [ ] `swissarmyhammer-entity/src/filter.rs` — unit test: insert/get typed values
- [ ] `swissarmyhammer-entity/src/filter.rs` — unit test: get missing type returns None
- [ ] `swissarmyhammer-entity/src/context.rs` — integration test: `list_where` with simple field predicate
- [ ] `swissarmyhammer-entity/src/context.rs` — integration test: `list_where` with context extra
- [ ] `swissarmyhammer-entity/src/context.rs` — integration test: predicate accesses `ctx.entities` for cross-entity logic
- [ ] `cargo nextest run -p swissarmyhammer-entity` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

#virtual-tags