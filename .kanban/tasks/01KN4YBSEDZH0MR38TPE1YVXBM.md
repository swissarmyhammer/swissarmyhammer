---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffb380
title: Fix (i) inspect button on task card — wrong command ID
---
## What

The (i) inspect button on task cards in the board view does nothing when clicked. The `InspectButton` component in `kanban-app/ui/src/components/entity-card.tsx` (line 164) resolves `"entity.inspect"` from the command scope, but the command is registered under the key `"ui.inspect"` (from `builtin/fields/entities/task.yaml` line 8). `resolveCommand` does exact `Map.get()` lookups, so it returns `null` and no action fires.

Double-click works because `focus-scope.tsx` line 197 correctly resolves `"ui.inspect"`.

### Fix
Change line 164 in `entity-card.tsx` from:
```typescript
const cmd = resolveCommand(scope, "entity.inspect");
```
to:
```typescript
const cmd = resolveCommand(scope, "ui.inspect");
```

### Files to modify
- `kanban-app/ui/src/components/entity-card.tsx` — change `"entity.inspect"` to `"ui.inspect"` in `InspectButton`

## Acceptance Criteria
- [ ] Clicking the (i) button on a task card opens the inspector
- [ ] Double-click still works as before

## Tests
- [ ] Test: verify `resolveCommand` is called with `"ui.inspect"` in entity-card tests
- [ ] Run: `pnpm test` in `kanban-app/ui/` — all pass