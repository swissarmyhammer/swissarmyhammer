---
assignees:
- claude-code
position_column: todo
position_ordinal: ff8880
title: Fix runaway drag-auto-scroll that mis-positions card drops in columns
---
## What

Two user-reported symptoms in the board view, both rooted in the same bug:

1. **"Odd scrolling behavior up the column"** — a column keeps scrolling after a drag leaves the column or after the drop completes. The scroll doesn't stop.
2. **"Drag and drop randomly mis-positions cards"** — the card lands in a different slot than the one the user visually released on. This happens because the runaway auto-scroll shifts the column contents under the pointer while the user is holding, so the `DropZone` under the pointer at release is not the one they aimed at.

### Root cause

In `kanban-app/ui/src/components/column-view.tsx`, `useScrollLoop` schedules a `requestAnimationFrame` tick that only exits when `dirRef.current === 0` (set by `stop()`) or the container unmounts. `stop()` is called from exactly one place during a live drag: the "middle of column" branch of `handleDragOver`. When the pointer:

- leaves the column entirely → `dragover` stops firing → rAF keeps scrolling.
- releases (drop) → `dragend`/`drop` fire on the card/zone but nothing tells the column's scroll loop to stop → rAF keeps scrolling after the drop completes.

Because auto-scroll continues after the drop is dispatched, the content under the pointer between `dragover` (when the user aimed) and `drop` (when they released) has shifted, so the `DropZone` hit at release is different from the one the user saw. That is the "random" mis-positioning.

### Fix

In `kanban-app/ui/src/components/column-view.tsx`:

- `useColumnDragScroll` must invoke `stop()` on:
  - `onDragLeave` of the column container (when the pointer exits the column's bounds, not a child element — mirror the `currentTarget.contains(relatedTarget)` guard already used in `DropZone.handleDragLeave`).
  - `onDrop` of the column container (so a drop inside the scroll zone immediately halts the rAF).
  - A window-level `dragend` listener installed while the scroll loop is active — this catches drags that terminate outside the column (Escape, drop on invalid target, cross-window drops).
- The container `div` rendered by `VirtualizedCardList` / `EmptyColumn` / `SmallCardList` / `VirtualColumn` (they share `CONTAINER_CLASS`) must carry the new `onDragLeave` and `onDrop` handlers alongside the existing `onDragOver`. `ColumnDragScroll` should expose them.
- The window-level `dragend` listener must be installed once per active scroll loop and cleaned up when the loop idles, so we don't leak listeners.

### Files to modify

- `kanban-app/ui/src/components/column-view.tsx` — extend `useScrollLoop` / `useColumnDragScroll` to own the three new stop triggers and return `handleDragLeave` + `handleDrop`; wire them onto the container in `EmptyColumn`, `SmallCardList`, and `VirtualColumn`.
- `kanban-app/ui/src/components/column-dragover.browser.test.tsx` — extend the minimal test harness to cover the stop-on-drop / stop-on-leave / stop-on-window-dragend contract (see Tests).

### Out of scope

- Resizing/redesigning the 12px `DropZone` hit area — the existing sizing is not the root cause once auto-scroll is well-behaved. File a separate task if precision is still insufficient after this fix.
- `SCROLL_ZONE` / `SCROLL_SPEED` tuning — leave constants alone; the fix is a lifecycle bug, not a threshold bug.
- Column-reorder drag (`@dnd-kit`) — this bug is in the HTML5 task drag path only.

## Acceptance Criteria

- [ ] Dragging a card into the top or bottom scroll zone of a column scrolls the column; dragging back to the middle immediately stops scrolling (existing behavior preserved).
- [ ] Dragging out of the column (pointer leaves the column's bounding rect) stops the auto-scroll before the next animation frame.
- [ ] Dropping a card on any `DropZone` inside the column — including one hit while auto-scrolling — stops the scroll loop immediately; the column does not continue to scroll after the drop lands.
- [ ] Pressing Escape or dropping on an invalid target (both produce `dragend`) stops the auto-scroll.
- [ ] The drop dispatched to `handleZoneDrop` corresponds to the `DropZone` under the pointer at the moment of release — no drift from a scroll that outlives the drop.
- [ ] No window-level `dragend` listener is left attached when no scroll loop is running (no listener leaks across drags).

## Tests

- [ ] Add test to `kanban-app/ui/src/components/column-dragover.browser.test.tsx`: construct a column with enough cards to scroll, dispatch a `dragover` in the top scroll zone to start the loop, then dispatch a `drop` on the container and assert `scrollTop` does not advance on the next two `requestAnimationFrame` ticks.
- [ ] Add test to the same file: start the scroll loop via a top-zone `dragover`, dispatch `dragleave` with `relatedTarget` outside the container, and assert the loop halts (no further `scrollBy`).
- [ ] Add test to the same file: start the scroll loop via a top-zone `dragover`, dispatch a `dragend` on `window`, and assert the loop halts.
- [ ] Add test to the same file: after the loop halts via any of the above, assert `window` has no lingering `dragend` listener installed by the hook (spy on `addEventListener`/`removeEventListener` counts).
- [ ] Command to run: `pnpm -C kanban-app/ui test --run column-dragover` — all tests pass.
- [ ] Manual verification (document in PR): drag a card into the top 40px of a tall column, hold until auto-scroll engages, drop — the column stops scrolling and the card lands at the zone under the pointer.

## Workflow

- Use `/tdd` — write the four new browser tests first (each will fail because the current `useScrollLoop` has no drop/dragleave/window-dragend wiring), then add the handlers and listener lifecycle to `useColumnDragScroll` to make them pass. #drag-and-drop #bug