---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffff9680
title: Wire EntityFilterContext into VirtualTagStrategy::matches signature
---
## What

`EntityFilterContext` was built in VT-0 but never wired in. The strategy trait should use it as the sole parameter to `matches()` — the context IS the evaluation environment.

### Signature change

```rust
// Before:
fn matches(&self, entity: &Entity, all_tasks: &[Entity], terminal_column_id: &str) -> bool;

// After:
fn matches(&self, ctx: &EntityFilterContext) -> bool;
```

The caller puts everything into the context:
- `ctx.entities` — all tasks (already on the struct)
- `ctx.entity` — the entity under evaluation (add this field to EntityFilterContext)
- Terminal column ID — injected via `ctx.insert::<String>(terminal_column_id)` or a newtype wrapper

### Files to modify

**swissarmyhammer-entity/src/filter.rs:**
- Add `pub entity: &'a Entity` field to `EntityFilterContext`
- Update `new()` to take both `entity` and `entities`

**swissarmyhammer-kanban/src/virtual_tags.rs:**
- Change `VirtualTagStrategy::matches(&self, entity, all_tasks, terminal_column_id)` → `matches(&self, ctx: &EntityFilterContext)`
- Change `VirtualTagRegistry::evaluate` signature accordingly
- Update all three strategies (ReadyStrategy, BlockedStrategy, BlockingStrategy) to read from ctx
- Update all tests

**swissarmyhammer-kanban/src/task_helpers.rs:**
- `enrich_all_task_entities` builds an `EntityFilterContext` per entity, inserts terminal column ID, passes to `registry.evaluate`

## Acceptance Criteria
- [ ] `VirtualTagStrategy::matches` takes only `&EntityFilterContext`
- [ ] `EntityFilterContext` has `entity` and `entities` fields
- [ ] Terminal column ID accessed via typed extras
- [ ] All three strategies updated
- [ ] All tests pass

## Tests
- [ ] Existing virtual tag strategy tests updated to use context
- [ ] `cargo nextest run -p swissarmyhammer-entity` passes
- [ ] `cargo nextest run -p swissarmyhammer-kanban` passes

#review-finding