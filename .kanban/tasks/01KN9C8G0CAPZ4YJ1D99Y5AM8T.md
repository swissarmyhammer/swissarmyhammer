---
assignees:
- claude-code
depends_on:
- 01KN9C5394341SWFR5E65YZV4W
position_column: done
position_ordinal: fffffffffffffffffffffffffe80
title: Auto-select perspective after creation — both \"+\" button and auto-create
---
## What

After `perspective.save` creates a new perspective, neither the "+" button handler nor the auto-create logic selects it. The user clicks "+", a perspective is created (once event propagation works), but nothing is selected — no immediate feedback.

### Fix approach

After `perspective.save` succeeds, dispatch `ui.perspective.set` with the new perspective's ID. This requires `perspective.save` to return the new perspective's ID in its response.

### Files to modify
- `kanban-app/ui/src/components/perspective-tab-bar.tsx` — `handleAdd` (line ~60): use `useDispatchCommand` to dispatch both commands with string names:
  ```typescript
  const dispatch = useDispatchCommand();
  // in handleAdd:
  const result = await dispatch("perspective.save", { args: { name: "Untitled" } });
  // extract ID from result, then:
  await dispatch("ui.perspective.set", { args: { id: newId } });
  ```
- `kanban-app/ui/src/lib/perspective-context.tsx` — auto-create effect (line ~88): same pattern — after creating "Default" perspective, set it as active using `useDispatchCommand`
- `swissarmyhammer-kanban/src/commands/perspective_commands.rs` — `SavePerspectiveCmd::execute()`: ensure the response includes the new perspective's ID (check if it already does)

### What success looks like
1. User clicks "+" → new "Untitled" perspective appears AND is immediately selected (active tab)
2. First mount with no perspectives → "Default" created AND selected automatically

## Acceptance Criteria
- [ ] "+" button creates perspective AND selects it immediately
- [ ] Auto-create on mount creates "Default" AND selects it
- [ ] `perspective.save` response includes the new perspective ID
- [ ] `setActivePerspectiveId` called with new ID after creation

## Tests
- [ ] `perspective-tab-bar.test.tsx` — "+" click dispatches save then sets active perspective
- [ ] `perspective-context.test.tsx` — auto-create dispatches save then sets active
- [ ] `pnpm test` from `kanban-app/ui/` — all pass