---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffdd80
title: 'Test: task card drag reorder within a column'
---
## What

Regression test suite for dragging a task card to a new position within the same column (reorder).

## Why

No tests currently verify that task card reorder works end-to-end when `FileDropProvider` is active. The global `preventDefault` bug proved these scenarios are fragile and need permanent guards.

## Key components under test

- `DraggableTaskCard` (sortable-task-card.tsx) — sets `draggable`, fires `onDragStart` with `application/x-swissarmyhammer-task` MIME
- `DropZone` (drop-zone.tsx) — thin horizontal targets between cards, calls `onDrop` with placement data
- `computeDropZones` (drop-zones.ts) — produces N+1 zones for N cards with correct `beforeId`/`afterId`
- `FileDropProvider` (file-drop-context.tsx) — must NOT interfere with task drags

## Tests to add (in `drop-zone.test.tsx` or new `task-drag-reorder.test.tsx`)

- [ ] Dragging card A over DropZone between cards B and C fires `onDrop` with correct `beforeId`/`afterId`
- [ ] `dragover` on a DropZone sets `dropEffect: \"move\"` (not `\"none\"`)
- [ ] `dragover` on a DropZone adds the active visual class/highlight
- [ ] `dragleave` on a DropZone removes the visual highlight
- [ ] Drop with `application/x-swissarmyhammer-task` MIME is accepted; drop without it is rejected
- [ ] All above work with `FileDropProvider` wrapping the component tree (integration)
- [ ] `computeDropZones` for 0, 1, 3 cards produces correct zone count and placement data

## Files

- `kanban-app/ui/src/components/drop-zone.test.tsx` — extend or create sibling
- `kanban-app/ui/src/lib/drop-zones.test.ts` — verify existing coverage, extend if gaps