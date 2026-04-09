---
assignees:
- claude-code
position_column: todo
position_ordinal: a980
title: Generic entity.add command — dynamic surfacing from view scope + field-default creation
---
## What

Add a fully generic entity creation mechanism: when a view for entity type X is in the scope chain, an "New {EntityType}" command automatically appears and creates an entity with field defaults. No per-type command code needed.

Uses the same prefix-rewrite pattern as `view.switch:*` and `board.switch:*`.

### 1. Expand `ViewInfo` to carry `entity_type`

**File:** `swissarmyhammer-kanban/src/scope_commands.rs`

```rust
pub struct ViewInfo {
    pub id: String,
    pub name: String,
    pub entity_type: Option<String>,  // ADD THIS
}
```

**File:** `kanban-app/src/commands.rs` — in the `DynamicSources` construction where `ViewDef` is mapped to `ViewInfo`, propagate `entity_type`:

```rust
ViewInfo {
    id: v.id.to_string(),
    name: v.name.clone(),
    entity_type: v.entity_type.clone(),  // ADD THIS
}
```

### 2. Emit dynamic `entity.add:{type}` in `commands_for_scope`

**File:** `swissarmyhammer-kanban/src/scope_commands.rs` — in the dynamic commands section (after view.switch generation), add:

When a `view:*` moniker is in the scope chain, find the matching view in `DynamicSources.views`, read its `entity_type`, and emit:

```rust
ResolvedCommand {
    id: format!("entity.add:{}", entity_type),
    name: format!("New {}", capitalize(entity_type)),
    group: "entity".to_string(),
    available: true,
    // ...
}
```

### 3. Add `entity.add:*` prefix handler in dispatch

**File:** `kanban-app/src/commands.rs` — in `dispatch_command_internal`, alongside the existing `view.switch:*` and `board.switch:*` rewrites, add:

```rust
if effective_cmd.starts_with("entity.add:") {
    let entity_type = effective_cmd.strip_prefix("entity.add:").unwrap();
    return generic_entity_add(entity_type, &args, &handle).await;
}
```

The `generic_entity_add` function:
- Gets `FieldsContext` from the board handle
- Looks up entity type definition to get field list
- Creates `Entity::new(entity_type, ulid::Ulid::new().to_string())`
- For each field in the entity def, reads `FieldDef.default` and sets it (if present)
- Overrides with any explicitly provided args (title, name, column, etc.)
- For entities with a `position_column` field: if no column arg provided, resolve to lowest-order column (reuse `resolve_column` pattern from `task/add.rs`)
- For entities with a `position_ordinal` field: compute append ordinal
- Calls `ectx.write(&entity)` to persist

### 4. Add `default:` values to field definition YAMLs

**Files:** `swissarmyhammer-kanban/builtin/definitions/`
- `title.yaml` — add `default: "Untitled"`
- `tag_name.yaml` — add `default: "new-tag"`
- `name.yaml` — add `default: "New item"`
- `body.yaml` — add `default: ""`
- `order.yaml` — add `default: 0`

These drive the generic creation — no hardcoded per-type defaults needed.

## Acceptance Criteria
- [ ] When a grid view for tags is active, command palette shows "New Tag" (`entity.add:tag`)
- [ ] When a grid view for projects is active, command palette shows "New Project" (`entity.add:project`)
- [ ] When a grid view for tasks is active, command palette shows "New Task" (`entity.add:task`)
- [ ] Dispatching `entity.add:tag` creates a tag entity with default field values
- [ ] Dispatching `entity.add:task` creates a task in the lowest-order column (no column arg needed)
- [ ] Dispatching `entity.add:task` with explicit `column` arg places task in that column
- [ ] Adding a new entity type YAML + grid view YAML automatically gets "New {Type}" — no Rust code needed

## Tests
- [ ] Test `commands_for_scope` with a view moniker in scope chain → emits `entity.add:{type}` command
- [ ] Test `commands_for_scope` without view moniker → no `entity.add` emitted
- [ ] Test generic entity creation with field defaults
- [ ] Test task creation via `entity.add:task` defaults to lowest-order column
- [ ] Run: `cargo test -p swissarmyhammer-kanban` and `cargo test -p kanban-app` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.