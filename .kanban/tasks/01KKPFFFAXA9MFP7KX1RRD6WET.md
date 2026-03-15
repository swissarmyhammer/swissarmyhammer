---
position_column: done
position_ordinal: c380
title: Move ordinal computation to backend
---
## What
`board-view.tsx` has `computeOrdinal()` which calculates fractional positioning strings (midpoint ordinals) client-side after drag-drop. This is business logic — the backend should own ordinal computation.

The `task.move` command already accepts an `ordinal` parameter. The frontend should stop computing ordinals and instead pass `before_id` / `after_id` (neighbors) to the backend, which computes the ordinal atomically.

**Files:**
- `kanban-app/ui/src/components/board-view.tsx` — remove `computeOrdinal()` and `midpointOrdinal()`, pass neighbor info to backend instead
- Backend `task.move` command may need a new parameter pattern: `{ column, before_id?, after_id? }` instead of requiring a pre-computed ordinal

## Acceptance Criteria
- [ ] No ordinal math in the frontend
- [ ] Drag-drop still works correctly
- [ ] Backend computes ordinal from neighbor context
- [ ] All tests pass