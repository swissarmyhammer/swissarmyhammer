---
assignees:
- claude-code
position_column: todo
position_ordinal: b880
title: 'Entity store: patch field in place instead of replacing entity on field-changed'
---
## What

`entity-field-changed` handler in App.tsx (lines 357-416) fetches the entire entity via `get_entity` and replaces it in the array with `.map()`. This creates a new array reference every time ANY field on ANY entity changes, causing all consumers (grid-view, board-view, etc.) to re-render and rebuild derived state like `cellMonikers`.

This triggers the grid cursor reset bug: `cellMonikers` rebuilds → `firstCellMoniker` gets a new reference → the initial-focus effect re-fires → cursor snaps to (0,0).

**Current flow (App.tsx:379-394):**
```tsx
invoke(\"get_entity\", { entityType, id })
  .then((bag) => {
    const entity = entityFromBag(bag);
    const replaceById = (entities) =>
      entities.map((e) => (e.id === id ? entity : e));
    setEntitiesFor(entity_type, replaceById);
  });
```

**Target flow:** Patch only the changed fields on the existing entity object. The event payload already carries `changes: Array<{ field, value }>`. Use those directly instead of re-fetching:

```tsx
setEntitiesFor(entity_type, (entities) =>
  entities.map((e) => {
    if (e.id !== id) return e;
    const fields = { ...e.fields };
    for (const { field, value } of changes) {
      fields[field] = value;
    }
    return { ...e, fields };
  }),
);
```

This creates a new entity object only for the changed entity, and a new array only when an entity in it actually changed. All other entity references stay stable → `cellMonikers` doesn't rebuild → cursor doesn't reset.

**Files to modify:**
- `kanban-app/ui/src/App.tsx` — entity-field-changed handler (lines 357-416): use `changes` from event payload to patch fields in place, remove `get_entity` fetch

**Risk:** The `get_entity` fetch was added as a safety net (\"events are signals to re-fetch\"). Removing it means trusting the event payload carries correct field values. Verify that the Rust event emission includes actual values in the `changes` array.

## Acceptance Criteria
- [ ] entity-field-changed patches fields from event payload, no `get_entity` round-trip
- [ ] Entity array reference only changes when an entity in it actually changed
- [ ] Grid cursor stays put after editing a field (color picker, text, etc.)
- [ ] Board/column/swimlane entity updates still propagate to `setBoard`

## Tests
- [ ] Verify Rust emits `changes` with field values (check `kanban-app/src/commands.rs` event emission)
- [ ] `cd kanban-app/ui && pnpm vitest run` — all pass
- [ ] Manual: grid view on tags — edit color, verify cursor stays on same cell
- [ ] Manual: edit task title in board view, verify other windows update