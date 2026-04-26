---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffb380
title: Introduce `field:` moniker namespace in moniker.ts and Rust parse_moniker
---
## What

Change the field-row moniker format from `task:01ABC.body` to `field:task:01ABC.body` so field-row scopes don't masquerade as entity monikers in the scope chain. This is the foundational change that all other #field-moniker-fix cards depend on.

### Current format
- `fieldMoniker("task", "abc", "title")` → `"task:abc.title"`
- `parseMoniker("task:abc.title")` → `{ type: "task", id: "abc", field: "title" }`
- Backend `parse_moniker("task:abc.title")` → `("task", "abc.title")`

### New format
- `fieldMoniker("task", "abc", "title")` → `"field:task:abc.title"`
- `parseMoniker("field:task:abc.title")` → `{ type: "field", id: "task:abc", field: "title" }` — OR better, add a dedicated `parseFieldMoniker` that returns `{ entityType, entityId, field }`
- Backend `parse_moniker("field:task:abc.title")` → `("field", "task:abc.title")` — naturally skipped by `resolve_entity_id("task")` since type is `"field"`

### Files to modify

1. `kanban-app/ui/src/lib/moniker.ts` — Change `fieldMoniker()` to prepend `field:`. Add `parseFieldMoniker()` that extracts `{ entityType, entityId, field }` from `"field:type:id.field"` format. Keep `parseMoniker()` working for both entity and field monikers.
2. `kanban-app/ui/src/lib/moniker.test.ts` — Update tests for new format, add `parseFieldMoniker` tests.

### Design note

The `field:` prefix means `parse_moniker` on the Rust side returns `("field", "task:abc.title")`. Code that walks the scope chain checking `has_in_scope("task")` will naturally skip field monikers — which is exactly the fix we need. No Rust changes required for `parse_moniker` itself.

## Acceptance Criteria

- [ ] `fieldMoniker("task", "abc", "title")` returns `"field:task:abc.title"`
- [ ] A new `parseFieldMoniker("field:task:abc.title")` returns `{ entityType: "task", entityId: "abc", field: "title" }`
- [ ] `parseMoniker("field:task:abc.title")` returns `{ type: "field", id: "task:abc", field: "title" }` (backward-compatible, no throw)
- [ ] `parseMoniker("task:abc")` still works unchanged
- [ ] Backend `parse_moniker("field:task:abc.title")` returns `("field", "task:abc.title")` — no Rust changes needed

## Tests

- [ ] `kanban-app/ui/src/lib/moniker.test.ts` — Update `fieldMoniker` test, add `parseFieldMoniker` tests
- [ ] Run `cd kanban-app/ui && npx vitest run moniker` — passes

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.