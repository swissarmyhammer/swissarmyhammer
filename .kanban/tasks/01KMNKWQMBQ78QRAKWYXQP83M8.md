---
assignees:
- claude-code
depends_on:
- 01KMNKW4Y3JHWNN73J22HZJM7Y
position_column: done
position_ordinal: ffffffffffffffa180
title: Wire DropZones into ColumnView — replace midpoint computation
---
## What

Refactor `kanban-app/ui/src/components/column-view.tsx` to render `DropZone` components between cards instead of computing insert indices from mouse Y positions.

### Current flow (delete)
1. `computeInsertIndex()` — compares clientY to card midpoints → index
2. `insertAtIndex` / `localInsert` state → renders a colored bar at that index
3. `handleDragOver` — midpoint computation + auto-scroll
4. `handleDragLeave` — clears drag state
5. `handleDrop` — passes index to parent
6. `isDragOver` / `isDragTarget` state — dashed column border

### New flow (replace with)
1. `computeDropZones(taskIds, columnId, boardPath)` called at render time → zone descriptors
2. `DropZone` components rendered between each card (and one filling the column if empty)
3. Each zone handles its own dragover/drop events — no column-level drag handling
4. Zone's `onDrop` fires with its preconfigured descriptor → parent handles move

### Layout change
```
Before:                          After:
<div onDragOver onDrop           <div>
     isDragTarget border-dashed>   <DropZone before=A />
  {insertBar at index}             <Card A />
  <Card A />                       <DropZone before=B />
  <Card B />                       <Card B />
  <Card C />                       <DropZone before=C />
  {insertBar at end}               <Card C />
</div>                             <DropZone after=C />
                                 </div>

Empty column:                    Empty column:
<div isDragTarget border-dashed> <div>
  <Inbox /> No tasks               <DropZone variant=empty-column />
</div>                           </div>
```

### Props interface changes
- **Remove**: `insertAtIndex`, `onDragOver`, `onDragEnter`, `onDragLeave`, `isDragTarget`
- **Change**: `onDrop` signature from `(columnId, taskData, insertIndex)` to `(descriptor, taskData)`
- **Add**: `dragTaskId` — passed to DropZones for no-op hiding
- **Add**: `boardPath` — needed by `computeDropZones`
- **Add**: `dragActive` — passed to DropZones so they expand during any drag
- **Keep**: `containerRef` (for cross-window hit-testing), auto-scroll (DropZones still need scroll when near edges)

### What to delete
- `computeInsertIndex()` function
- `localInsert` state, `isDragOver` state
- `insertAtIndex` prop, `isDragTarget` prop
- `handleDragOver`, `handleDragLeave`, `handleDrop` callbacks
- `showDashes` variable and dashed border rendering
- The conditional insertion bar `<div className=\"h-1 bg-primary ...\" />`
- The empty column `<Inbox />` placeholder (moved into DropZone empty-column variant)

### Auto-scroll
Auto-scroll during drag still needed for long columns. Move the scroll-zone logic into the column container's `onDragOver` (lightweight — just checks Y position for scroll edges, no midpoint computation).

### Files
- **Modify**: `kanban-app/ui/src/components/column-view.tsx`

## Acceptance Criteria
- [ ] `computeInsertIndex` function is deleted
- [ ] No `insertIndex`, `isDragOver`, `isDragTarget`, or dashed border logic remains
- [ ] DropZones render between every pair of cards + at top and bottom
- [ ] Empty column renders a single `empty-column` variant DropZone
- [ ] Column auto-scroll during drag still works
- [ ] No column-level drag event handlers remain (zones handle their own events)

## Tests
- [ ] `kanban-app/ui/src/components/column-view.test.tsx` — column with 3 tasks renders 4 drop zones
- [ ] `kanban-app/ui/src/components/column-view.test.tsx` — drop zones have correct before/after attributes
- [ ] `kanban-app/ui/src/components/column-view.test.tsx` — empty column renders 1 drop zone with data-drop-empty
- [ ] `pnpm vitest run src/components/column-view.test.tsx` passes