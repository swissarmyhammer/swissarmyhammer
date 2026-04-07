---
assignees:
- claude-code
depends_on:
- 01KNJD7VB0QC38W3EETA84E15Y
position_column: done
position_ordinal: ffffffffffffffffffffff8780
position_swimlane: null
title: 'FILTER-2: Server-side filter evaluation in entity listing'
---
## What

Add an optional `filter` param to the `list_entities` Tauri command. When present, parse the DSL expression and evaluate it server-side against each entity before returning results. The frontend passes the active perspective's filter string.

### Files to modify
- `kanban-app/src/commands.rs` — `list_entities()`: add `filter: Option<String>` param. When `Some`, parse via `filter_expr::parse()`, then evaluate against each entity, returning only matches.
- `swissarmyhammer-filter-expr/src/eval.rs` — add `EntityFilterAdapter` that maps DSL atoms to entity fields:
  - `#tag-name` → entity `tags` field (array) contains tag. Also checks `virtual_tags`.
  - `@user-name` → entity `assignees` field matches
  - `^card-ref` → entity `depends_on` contains ref, or entity `id` matches
- `kanban-app/ui/src/lib/entity-store-context.tsx` (or wherever `list_entities` is called) — pass filter string from active perspective

### Error handling
- If the filter param fails to parse, return an error (the UI already validated on save, so this is a defensive check)
- Empty string filter treated as no filter

### Scope
This card is Rust-side only + wiring the param from the frontend call site. Removing the old client-side JS eval code is in FILTER-6.

## Acceptance Criteria
- [ ] `list_entities("task", filter: Some("#bug"))` returns only entities tagged "bug"
- [ ] `list_entities("task", filter: Some("#READY"))` matches virtual tags
- [ ] `list_entities("task", filter: Some("#bug && @will"))` applies boolean logic
- [ ] `list_entities("task", filter: None)` returns all entities (unchanged behavior)
- [ ] Performance: filtering 10k entities in <10ms

## Tests
- [ ] `swissarmyhammer-filter-expr/src/eval.rs` — entity adapter unit tests with real Entity structs
- [ ] `kanban-app/src/commands.rs` — integration test: list_entities with filter returns subset
- [ ] `cargo test` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.