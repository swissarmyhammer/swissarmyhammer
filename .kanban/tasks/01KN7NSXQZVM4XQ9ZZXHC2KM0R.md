---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffcc80
title: 'Test: task card drag between columns (cross-column move)'
---
## What

Regression test suite for dragging a task card from one column to another on the same board.

## Why

Cross-column move involves the ColumnView `handleContainerDragOver` (which has the `Files` type check), DropZones in the target column, and the `BoardView.handleZoneDrop` handler that calls `persistMove`. None of this is tested as an integrated flow.

## Key components under test

- `ColumnView.handleContainerDragOver` (column-view.tsx:395) — must `preventDefault` for task drags, must NOT for file drags
- `DropZone` in target column — accepts the drop, emits placement + column info
- `BoardView.handleZoneDrop` (board-view.tsx:420) — detects same-board drop, calls `persistMove`
- `FileDropProvider` — must not interfere

## Tests to add

- [ ] `ColumnView` dragover with `application/x-swissarmyhammer-task` type calls `preventDefault` and sets `dropEffect: \"move\"`
- [ ] `ColumnView` dragover with `Files` type does NOT call `preventDefault` (returns early)
- [ ] Dropping a task card on a DropZone in a different column fires `onDrop` with the target column ID
- [ ] `handleZoneDrop` with matching board path calls `persistMove` (not cross-board session)
- [ ] Auto-scroll activates when dragging near top/bottom edges of column
- [ ] All above work with `FileDropProvider` wrapping (integration)

## Files

- `kanban-app/ui/src/components/column-view.test.tsx` — new or extend
- `kanban-app/ui/src/components/board-view.test.tsx` — new or extend