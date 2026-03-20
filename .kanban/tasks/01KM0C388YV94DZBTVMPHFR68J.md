---
assignees:
- claude-code
depends_on:
- 01KM0C2NDHKEB7QXG4XEAQ0KR5
position_column: done
position_ordinal: '8280'
title: Handle cross-window drop via DragDropEvent
---
## What
In target windows, listen for Tauri's `DragDropEvent` to detect when an OS drag enters/hovers/drops. Use the event's position coordinates to hit-test which column the drop landed on, then call `complete_drag_session` with that column.

**Files:**
- `kanban-app/ui/src/components/cross-window-drop-overlay.tsx` — major rework to use DragDropEvent instead of mousemove
- `kanban-app/ui/src/lib/drag-session-context.tsx` — may need to listen for DragDropEvent here
- `kanban-app/ui/src/components/board-view.tsx` — pass column element refs to overlay

**Approach:**
Tauri emits `DragDropEvent` with variants:
- `Enter { paths, position }` — drag entered the webview
- `Over { position }` — drag is moving over the webview
- `Drop { paths, position }` — drag was released
- `Leave` — drag left without dropping

Since we're using `DragItem::Data` (not files), `paths` will be empty. But `position` gives us the cursor coordinates.

**Implementation:**
1. Listen for `tauri://drag-enter` and `tauri://drag-over` events in the overlay
2. On each event, hit-test `position` against column element rects (existing `columnRefs` pattern)
3. Highlight the hovered column (existing visual behavior)
4. On `tauri://drag-drop`, complete the session: call `completeSession(hoveredColumn, { ... })`
5. On `tauri://drag-leave`, clear highlights

**Position coordinate issue:** There's a known ~28px y-offset on macOS (Tauri issue #10744). May need to compensate. Test empirically.

**Key difference from current overlay:** The current overlay uses `mousemove` events which don't fire during OS drags (the OS owns the pointer). `DragDropEvent` is the only way to get position during an OS drag.

## Acceptance Criteria
- [ ] Target window highlights correct column during OS drag hover
- [ ] Dropping in target window completes the drag session
- [ ] Task appears in the target column after drop
- [ ] Dragging off the window (without dropping) clears highlights
- [ ] Copy mode (Alt/Option held during drop) still works

## Tests
- [ ] Manual test: drag from window A, hover over window B columns — highlights work
- [ ] Manual test: drop in window B — task moves to correct column
- [ ] Manual test: drag over window B then drag away — highlights clear
- [ ] Manual test: hold Alt during drop — task is copied not moved
- [ ] `cargo nextest run` — no regressions