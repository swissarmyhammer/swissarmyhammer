---
assignees:
- claude-code
depends_on:
- 01KKWF8YVWQ4XNC24DV8YCK650
position_column: done
position_ordinal: ffffffffca80
title: Target window drop overlay and cross-window drop handling
---
## What
When a remote drag session is active, show drop zone overlays in target windows and handle the drop on mouseup.

**Files:**
- `kanban-app/ui/src/components/cross-window-drop-overlay.tsx` (new) — full-window overlay with column drop zones
- `kanban-app/ui/src/components/board-view.tsx` — render CrossWindowDropOverlay when isRemoteDragActive

**CrossWindowDropOverlay component:**
- Renders when `isRemoteDragActive` is true (from useDragSession)
- Full-window semi-transparent overlay (pointer-events: all to capture mouse)
- Uses board column layout to position drop zones over each column
- Highlights the column the pointer is currently over
- Shows ghost card preview (rendered from entitySnapshot) following cursor
- Uses existing EntityCard component for the ghost preview (same as DragOverlay in board-view)

**Pointer tracking:**
- `pointermove` on the overlay maps cursor X to column index
- `pointermove` Y position determines before/after which task in that column
- Visual indicator shows insertion point (line between cards)

**Drop handling (pointerup/mouseup):**
- Determine target column and position (before_id / after_id)
- Check if Alt/Option key is held → copy mode
- Call `completeDragSession(targetColumn, beforeId, afterId, copy)`
- Backend handles same-board vs cross-board routing

**Edge cases:**
- Drag session cancelled while overlay showing → overlay disappears
- Drag session completed by another window → overlay disappears  
- Window showing same board as source → same-board move (simpler, no transfer needed)
- Pointer leaves target window without dropping → overlay stays until session cancelled or completed elsewhere

**Alt/Option visual feedback:**
- When Alt held, show "+" badge on ghost card to indicate copy mode
- When Alt released, badge disappears

## Acceptance Criteria
- [ ] Overlay appears in target window when remote drag is active
- [ ] Column drop zones highlight on hover
- [ ] Ghost card preview follows cursor using entity snapshot data
- [ ] Mouseup on column executes completeDragSession with correct column and position
- [ ] Alt key toggles copy mode with visual indicator
- [ ] Overlay disappears on cancel/complete events
- [ ] Same-board cross-window drop works (move between columns)
- [ ] Cross-board drop works (transfer to different board)

## Tests
- [ ] Manual: drag from window A → overlay appears in window B with column highlights
- [ ] Manual: drop on column in window B → card appears in correct column
- [ ] Manual: hold Alt + drop → card copied (exists in both boards)
- [ ] Manual: press Escape in source window → overlay disappears in target
- [ ] `npm run build` compiles without errors