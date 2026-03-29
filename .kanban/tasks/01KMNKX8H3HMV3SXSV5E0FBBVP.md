---
assignees:
- claude-code
depends_on:
- 01KMNKWQMBQ78QRAKWYXQP83M8
position_column: done
position_ordinal: ffffffffffffcc80
title: Simplify BoardView drop handler — zones provide placement directly
---
## What

Refactor `kanban-app/ui/src/components/board-view.tsx` to consume `DropZoneDescriptor` from zones instead of computing placement from insert indices. Also remove all column-level drag handlers that are now redundant.

### Current flow (delete)
1. `handleColumnDragOverHTML5(columnId, insertIndex)` — tracks drag state with index
2. `handleColumnDragEnter` / `handleColumnDragLeave` — tracks which column is drag target
3. `handleTaskDrop(columnId, taskData, insertIndex)` — computes `before_id`/`after_id` from index + sourceIndex heuristic
4. `computePlacement()` import from `drop-placement.ts`
5. `taskDrag` state with `targetColumn` and `insertIndex`

### New flow (replace with)
1. `handleZoneDrop(descriptor: DropZoneDescriptor, taskData: string)` — receives preconfigured placement
2. For same-board: `persistMove(taskId, descriptor.columnId, entity, { before: descriptor.beforeId, after: descriptor.afterId })`
3. For cross-board: `completeSession(descriptor.columnId, { beforeId: descriptor.beforeId, afterId: descriptor.afterId })`

### What to delete from board-view.tsx
- `handleColumnDragOverHTML5` callback
- `handleColumnDragEnter` / `handleColumnDragLeave` callbacks
- `handleTaskDrop` callback (replaced by `handleZoneDrop`)
- `insertIndex` and `targetColumn` fields in `taskDrag` state
- Import of `computePlacement` from `drop-placement.ts`
- Props passed to ColumnView: `onDragOver`, `onDragEnter`, `onDragLeave`, `insertAtIndex`, `isDragTarget`

### What to add
- `handleZoneDrop` callback — dead simple, just reads descriptor
- Props to ColumnView: `onDrop` (new signature), `dragTaskId`, `dragActive`, `boardPath`

### Simplified `taskDrag` state
```ts
// Before:
{ sourceTaskId, sourceColumn, targetColumn, insertIndex }

// After:
{ sourceTaskId, sourceColumn }
```

### Files
- **Modify**: `kanban-app/ui/src/components/board-view.tsx`

## Acceptance Criteria
- [ ] No `insertIndex`, `sourceIndex`, `targetColumn`, or placement computation in board-view
- [ ] No column-level drag enter/leave/over handlers
- [ ] Same-board drop uses descriptor's before/after directly
- [ ] Cross-board drop passes descriptor's before/after to `completeSession`
- [ ] `taskDrag` state only tracks `sourceTaskId` and `sourceColumn`
- [ ] All existing drag UX (drag ghost, cross-window session) still works

## Tests
- [ ] `pnpm vitest run` — full suite passes (no regressions)
- [ ] Backend Rust tests still pass: `cargo nextest run -p swissarmyhammer-kanban`