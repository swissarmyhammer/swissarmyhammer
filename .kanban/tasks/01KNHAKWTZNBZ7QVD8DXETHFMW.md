---
assignees:
- claude-code
position_column: todo
position_ordinal: 7d80
title: 'Bug: Column drag-and-drop snaps back before entity refresh arrives'
---
## What

After dragging a column to a new position and dropping, the column visually snaps back to its original position before eventually settling into the correct order (if at all). The backend `column.reorder` command updates the `order` field correctly (user can see numbers changing), but the visual update is lost because `virtualColumnOrder` is cleared before the entity store refresh propagates.

### Root cause

In `kanban-app/ui/src/components/board-view.tsx`, `handleColumnDragEnd` (line 344) dispatches `column.reorder` and then immediately calls `setVirtualColumnOrder(null)` in the `finally` block (line 371). This reverts the column rendering to `columnIdList`, which is derived from `board.columns` sorted by `order`. But `board.columns` hasn't updated yet — the file watcher → IPC event → React state pipeline takes a few hundred milliseconds.

### Fix

Keep `virtualColumnOrder` alive after a successful reorder, and only clear it when `columnIdList` changes (meaning the real data caught up). This prevents the snap-back.

### Files to modify

1. **`kanban-app/ui/src/components/board-view.tsx`** (`handleColumnDragEnd`, ~line 344)
   - On success: do NOT call `setVirtualColumnOrder(null)` — leave it in place so columns stay in the dragged position
   - On error/cancel: still clear `virtualColumnOrder` to revert
   - Add a `useEffect` that watches `columnIdList` and clears `virtualColumnOrder` when the real data arrives matching the virtual order (or any time `columnIdList` changes)

### Detailed change

```tsx
// After the dispatch succeeds, DON'T clear:
try {
  await dispatch(\"column.reorder\", { args: { id: activeId, target_index: newIndex } });
  // virtualColumnOrder stays — columns remain in dragged position
} catch (e) {
  console.error(\"Failed to reorder columns:\", e);
  setVirtualColumnOrder(null); // revert on error
} finally {
  setActiveColumn(null); // always clear the drag overlay
}

// New effect: clear virtual order when real data catches up
useEffect(() => {
  setVirtualColumnOrder(null);
}, [columnIdList]);
```

## Acceptance Criteria

- [ ] Dragging a column to a new position and dropping keeps the column visually in the new position (no snap-back)
- [ ] When the entity store refresh arrives, the transition is seamless (virtual order → real order with no flicker)
- [ ] Cancelling a drag (Escape or dropping outside) still reverts to the original position
- [ ] Failed reorder (backend error) reverts to the original position
- [ ] `npm test` passes

## Tests

- [ ] `kanban-app/ui/src/components/board-view.test.tsx` — add test: after successful column reorder dispatch, columns remain in dragged order until board data updates
- [ ] `kanban-app/ui/src/components/board-view.test.tsx` — add test: failed column reorder dispatch reverts to original order
- [ ] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.