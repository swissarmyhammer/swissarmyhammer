---
assignees:
- claude-code
depends_on:
- 01KM0C0EZ8PN6YDDKW81QAM4CZ
- 01KM0C1FK7M3D8QM6YQBH259RX
- 01KM0C20RZ2K8F3S498TT1V0MV
position_column: done
position_ordinal: ffffffee80
title: Wire frontend drag start to OS drag + @dnd-kit hybrid
---
## What
Modify the frontend drag initiation so that when a task drag begins, it simultaneously starts both the @dnd-kit intra-window drag AND the OS-level drag via `start_os_drag`. The two systems coexist: @dnd-kit handles sorting within the source window, while the OS drag provides the cross-window ghost.

**Files:**
- `kanban-app/ui/src/components/board-view.tsx` — modify `handleDragStart`
- `kanban-app/ui/src/components/sortable-task-card.tsx` — may need to pass card DOM ref for image capture
- `kanban-app/ui/src/lib/drag-session-context.tsx` — update `startSession` to also trigger OS drag

**Approach:**
In `handleDragStart`:
1. @dnd-kit fires `onDragStart` as before (virtual layout, active task state)
2. Capture the card DOM element as an image (from card 4)
3. Call `invoke('start_os_drag', { taskId, taskFields, windowLabel, previewImage })` 
4. The OS drag and @dnd-kit drag run concurrently

**Key challenge:** @dnd-kit uses `PointerSensor` which captures pointer events. When the pointer leaves the window, @dnd-kit will fire `onDragEnd` with no `over` target (a cancel). The OS drag continues. We need to handle this gracefully:
- In `handleDragEnd`: if no `over` target and an OS drag session is active, do NOT cancel the session — the drop might happen in another window
- The session completion/cancellation is handled by the target window or by Escape

**Another consideration:** `drag::start_drag()` is blocking and takes control of the event loop on macOS. This may conflict with @dnd-kit's PointerSensor. We may need to start the OS drag slightly differently — possibly on `mousedown` before @dnd-kit activates, or as a fallback when the pointer leaves the window boundary.

This card may need to be split further during implementation if the @dnd-kit + OS drag coexistence proves complex.

## Acceptance Criteria
- [ ] Dragging a task within a window still works via @dnd-kit (sorting, column moves)
- [ ] When pointer leaves the source window, OS drag ghost is visible
- [ ] @dnd-kit cancel (pointer leaves window) does NOT cancel the cross-window session
- [ ] Pressing Escape cancels both @dnd-kit and OS drag

## Tests
- [ ] Manual test: drag task within same window — @dnd-kit sorting works as before
- [ ] Manual test: start drag, move pointer outside window — OS ghost visible, no crash
- [ ] Manual test: drag to another window then back — source window @dnd-kit recovers
- [ ] `cargo nextest run` — no regressions