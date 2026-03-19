---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffea80
title: Fix intra-window collision detection
---
## What
Replace `closestCorners` collision detection in `board-view.tsx` with a pointer-aware strategy so the drop target matches the visual highlight.

The current `closestCorners` algorithm measures distance from all 4 corners of the dragged element's bounding rect. This means the user has to drag further than expected — the highlight shows the correct target but the collision engine disagrees until the full bounding box crosses over.

**Files:**
- `kanban-app/ui/src/components/board-view.tsx` — change `collisionDetection={closestCorners}` to a custom hybrid

**Approach:**
Use `pointerWithin` for detecting which column the pointer is over (container-level), combined with `closestCenter` for ordering within a column (item-level). `@dnd-kit` supports custom collision detection functions — write a hybrid that uses pointer position for container detection and closest-center for item ordering. This is the standard pattern for Kanban boards with this library.

```tsx
// Custom collision detection:
// 1. Use pointerWithin to find which droppable containers the pointer is inside
// 2. Filter to column drop zones
// 3. Within that column, use closestCenter on the task items
```

## Acceptance Criteria
- [ ] Dropping a task into a highlighted column always lands where the highlight shows
- [ ] No more \"drag further than expected\" behavior
- [ ] Column reordering (horizontal drag) still works correctly
- [ ] Task reordering within a column still works correctly

## Tests
- [ ] Manual test: drag a task to a column boundary — drop should register as soon as the highlight appears
- [ ] Manual test: drag a task within the same column — reordering still works
- [ ] Manual test: drag a column left/right — reordering still works
- [ ] `cargo nextest run` — no regressions