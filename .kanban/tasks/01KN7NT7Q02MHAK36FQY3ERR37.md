---
assignees:
- claude-code
position_column: todo
position_ordinal: '8380'
title: 'Test: task card cross-board drag (between windows)'
---
## What

Regression test suite for dragging a task card from one board window to another (cross-board/cross-window drag).

## Why

Cross-board drag uses the `DragSession` system (Tauri events: `drag-session-active`, `drag-session-cancelled`, `drag-session-completed`) and backend commands (`drag.start`, `drag.cancel`, `drag.complete`). The global `FileDropProvider` bug caused `dropEffect` misdetection that broke session lifecycle. Existing tests in `drag-session-context.test.tsx` test the session in isolation but not the full flow.

## Key components under test

- `DragSessionProvider` (drag-session-context.tsx) — manages session state, emits/listens Tauri events
- `BoardView.handleTaskDragStart` (board-view.tsx:394) — starts backend session
- `BoardView.handleTaskDragEnd` (board-view.tsx:406) — cancels session if `dropEffect === \"none\"`
- `BoardView.handleZoneDrop` (board-view.tsx:420) — detects cross-board drop (different board path), calls `completeSession`
- `FileDropProvider` — must not corrupt `dropEffect` detection

## Tests to add

- [ ] `handleTaskDragStart` calls `drag.start` backend command with correct task entity and source window
- [ ] `handleTaskDragEnd` with `dropEffect: \"none\"` calls `drag.cancel`
- [ ] `handleTaskDragEnd` with `dropEffect: \"move\"` does NOT call cancel (drop was accepted somewhere)
- [ ] `handleZoneDrop` with different board path calls `completeSession` (not `persistMove`)
- [ ] `DragSessionProvider` sets `isSessionActive` when receiving `drag-session-active` Tauri event from different window
- [ ] Receiving `drag-session-completed` clears the active session
- [ ] `dropEffect` is correctly `\"none\"` when drag ends outside any valid drop target (with FileDropProvider active — this is the regression case)

## Files

- `kanban-app/ui/src/lib/drag-session-context.test.tsx` — extend with integration scenarios
- `kanban-app/ui/src/components/board-view.test.tsx` — new or extend