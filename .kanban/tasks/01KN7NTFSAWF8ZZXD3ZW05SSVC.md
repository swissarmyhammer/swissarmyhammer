---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffe680
title: 'Test: column reorder drag (dnd-kit)'
---
## What

Regression test suite for dragging columns left/right to reorder them on the board.

## Why

Column reorder uses `@dnd-kit/core` with `PointerSensor` (distance: 5 activation). It coexists with both the HTML5 task card drag and the FileDropProvider. No tests currently verify this works when the other drag systems are active.

## Key components under test

- `BoardView` DndContext (board-view.tsx:480) — wraps column layout, handles `onDragStart/onDragOver/onDragEnd`
- `SortableColumn` (sortable-column.tsx) — uses `useSortable` from `@dnd-kit/sortable`, drag handle is `GripHorizontal` button
- `PointerSensor` activation constraint — `distance: 5` prevents accidental drags

## Tests to add

- [ ] Dragging the column grip handle initiates a dnd-kit drag (overlay appears)
- [ ] Dragging a column over another column reorders them (onDragEnd fires with correct active/over IDs)
- [ ] Clicking the column grip handle without moving 5px does NOT start a drag
- [ ] Column drag does NOT interfere with task card drag (dragging a task card does not trigger column reorder)
- [ ] Column drag works with FileDropProvider active
- [ ] DragOverlay renders the column name during drag

## Files

- `kanban-app/ui/src/components/board-view.test.tsx` — new or extend
- `kanban-app/ui/src/components/sortable-column.test.tsx` — new or extend