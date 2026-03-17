---
assignees:
- claude-code
depends_on:
- 01KKWF8YVWQ4XNC24DV8YCK650
position_column: done
position_ordinal: ffffffcf80
title: Source window drag integration with @dnd-kit
---
## What
Modify board-view.tsx to wire the @dnd-kit drag lifecycle into the drag session system. When a drag starts, register with the backend. When the pointer leaves the window, cancel @dnd-kit but keep the backend session alive.

**Files:**
- `kanban-app/ui/src/components/board-view.tsx` — modify handleDragStart, handleDragEnd, add pointer tracking

**handleDragStart changes:**
- After existing @dnd-kit setup, call `startDragSession(entityType, entityId)` to register with backend
- This runs in parallel — the @dnd-kit drag proceeds normally for within-window moves

**Pointer exit detection:**
- During active drag, add `pointermove` listener on document
- Use `getCurrentWindow().innerPosition()` and `getCurrentWindow().innerSize()` to get window bounds
- When pointer position exits bounds, programmatically cancel @dnd-kit drag (set activeDrag state to null, clean up virtual layout)
- Do NOT cancel the backend drag session — it stays active for target windows

**handleDragEnd changes:**
- If drag completes normally within window (drop on valid target), call `cancelDragSession()` to clean up backend session since it was handled locally by existing persistMove logic
- Existing persistMove logic unchanged

**handleDragCancel changes:**
- If @dnd-kit fires cancel AND pointer is inside window bounds → cancel backend session too (user pressed Escape)
- If pointer is outside bounds → leave backend session active (pointer left for another window)

**Alt/Option key tracking:**
- Track modifier key state during drag (altKey on pointer events)
- Pass `copy: true` flag when Alt is held — but this is only used by the target window's completeDragSession call

## Acceptance Criteria
- [ ] Drag start registers backend session
- [ ] Normal within-window drop works exactly as before (no regression)
- [ ] Pointer leaving window cancels @dnd-kit but keeps backend session
- [ ] Escape key cancels both @dnd-kit and backend session
- [ ] No visual glitches during normal within-window drag

## Tests
- [ ] Manual: drag card within window → same behavior as before
- [ ] Manual: drag card out of window → @dnd-kit drag cancelled, card returns to original position in source window
- [ ] `npm run build` compiles without errors