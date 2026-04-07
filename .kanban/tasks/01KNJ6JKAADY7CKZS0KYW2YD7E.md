---
assignees:
- claude-code
depends_on:
- 01KNJ6HNYNBT3FAGDBYDEGNXPY
position_column: done
position_ordinal: ffffffffffffffffffffff8680
title: Migrate frontend field-moniker consumers to `field:` namespace
---
## What

Update all frontend code that constructs or consumes field-row monikers to use the new `field:type:id.field` format. Depends on the moniker.ts foundational change.

### Constructors (build field monikers)

1. **`kanban-app/ui/src/components/entity-inspector.tsx:267`** — `const scopeMoniker = \`${entity.moniker}.${field.name}\``
   Change to: use `fieldMoniker(entity.entity_type, entity.id, field.name)` or prefix with `field:`.

2. **`kanban-app/ui/src/components/grid-view.tsx:84,97`** — `const mk = \`${entities[r].moniker}.${columns[c].field.name}\``
   Same pattern, same fix.

### Consumers (read/match field monikers)

3. **`kanban-app/ui/src/components/entity-inspector.tsx:91`** — `fieldMonikers` array, used in `claimPredicates` for nav.up/down/left/right/first/last. All predicate `when` callbacks compare `f === fieldMonikers[i]` or call `isDescendantOf(fieldMonikers[i])`. These use exact string equality or scope-chain walking — both will work as-is once the monikers themselves change, since the FocusScope registry stores whatever moniker string is used.

4. **`kanban-app/ui/src/components/fields/displays/badge-list-display.tsx:70`** — `useParentFocusScope()` returns the nearest ancestor FocusScope's moniker. With the format change, this returns `"field:task:01ABC.tags_field"` instead of `"task:01ABC.tags_field"`. The pill navigation predicates at lines 93-113 compare against this value — they use exact equality so they'll work as long as both sides change consistently.

5. **`kanban-app/ui/src/components/grid-view.tsx:80-89`** — `cellMonikerMap` maps moniker string → `{row, col}`. The focused moniker lookup at line 102 must match the new format.

### Tests to update

- `kanban-app/ui/src/components/inspector-focus-bridge.test.tsx:177,193` — asserts `"task:test-id.title"`, change to `"field:task:test-id.title"`
- `kanban-app/ui/src/components/fields/displays/badge-list-nav.test.tsx` — uses `parentMoniker="field:tags"` in tests (already uses `field:` prefix — verify compatibility)
- `kanban-app/ui/src/components/entity-inspector.test.tsx` — any moniker format assertions

## Acceptance Criteria

- [ ] `entity-inspector.tsx` field-row FocusScopes use `field:` prefixed monikers
- [ ] `grid-view.tsx` cell FocusScopes use `field:` prefixed monikers
- [ ] Inspector field navigation (up/down/left/right) still works — predicates compare updated moniker strings
- [ ] Badge-list pill navigation still works — `useParentFocusScope()` returns `field:` moniker
- [ ] Grid cell focus tracking still works — `cellMonikerMap` uses updated format
- [ ] All inspector/grid/badge-list tests pass

## Tests

- [ ] `kanban-app/ui/src/components/inspector-focus-bridge.test.tsx` — Update moniker assertions
- [ ] `kanban-app/ui/src/components/fields/displays/badge-list-nav.test.tsx` — Verify pill nav tests pass
- [ ] `kanban-app/ui/src/components/entity-inspector.test.tsx` — Update any moniker assertions
- [ ] Run `cd kanban-app/ui && npx vitest run` — all tests pass

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass. #field-moniker-fix