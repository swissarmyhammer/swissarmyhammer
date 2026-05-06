---
assignees:
- claude-code
depends_on:
- 01KNHAQS6GGPEKH0X273147W3Y
position_column: done
position_ordinal: ffffffffffffffffffffffffffff9a80
title: 'Frontend: Use entity.moniker from backend instead of constructing monikers client-side'
---
## What

After the backend includes `moniker` in entity JSON (card 01KNHAQS6GGPEKH0X273147W3Y), update all frontend call sites that construct monikers from `entity.entity_type` + `entity.id` to use `entity.moniker` instead. This eliminates the UI as a source of truth for moniker format.

### Call sites to update

All files use `moniker()` from `kanban-app/ui/src/lib/moniker.ts`:

1. **`kanban-app/ui/src/components/entity-card.tsx`** (line 70):
   `moniker(entity.entity_type, entity.id)` → `entity.moniker`

2. **`kanban-app/ui/src/components/inspector-focus-bridge.tsx`** (line 34):
   `moniker(entity.entity_type, entity.id)` → `entity.moniker`

3. **`kanban-app/ui/src/components/data-table.tsx`** (line 383):
   `moniker(entity.entity_type, entity.id)` → `entity.moniker`

4. **`kanban-app/ui/src/components/entity-inspector.tsx`** (lines 94, 269):
   `fieldMoniker(entity.entity_type, entity.id, f.name)` — keep `fieldMoniker()` but derive from `entity.moniker + "." + field` or leave as-is since field monikers extend the entity moniker. The cleanest approach: use ``\`${entity.moniker}.${field}\` `` directly.

5. **`kanban-app/ui/src/components/command-palette.tsx`** (lines 241, 502):
   `moniker(result.entity_type, result.entity_id)` — these use search results, not `Entity` objects. Check if search results also carry a moniker field. If not, these stay as-is (search results are a different shape).

6. **`kanban-app/ui/src/components/data-table.test.tsx`** (line 114):
   ``target: \`${entity.entity_type}:${entity.id}\` `` → `target: entity.moniker`

### What NOT to change

- `moniker("board", "board")`, `moniker("column", col.id)`, `\`window:${label}\`` — these construct monikers for non-entity scopes (board, column, window, store, view, mode). They don't come from `Entity` objects. Leave them as-is.
- `moniker()` and `fieldMoniker()` utilities in `moniker.ts` — keep them for the non-entity cases above.

## Acceptance Criteria

- [ ] No `moniker(entity.entity_type, entity.id)` patterns remain in component files
- [ ] Entity-derived monikers read `entity.moniker` from the backend-provided field
- [ ] Non-entity monikers (board, column, window, store, view) still use the utility function
- [ ] All frontend tests pass

## Tests

- [ ] `kanban-app/ui/src/components/entity-card.test.tsx` — existing tests pass (moniker now from entity data)
- [ ] `kanban-app/ui/src/components/data-table.test.tsx` — update test fixture entities to include `moniker` field
- [ ] Run `npm test` in kanban-app — all pass

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.