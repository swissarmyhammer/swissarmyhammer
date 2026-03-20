---
position_column: done
position_ordinal: ff8180
title: Move column reorder logic to backend
---
## What
`column-reorder.ts` computes new order values for all affected columns after a drag-drop reorder. This is schema/database logic — the frontend shouldn't understand the column ordering scheme.

**Files:**
- `kanban-app/ui/src/lib/column-reorder.ts` — remove or reduce to just calling backend
- `kanban-app/ui/src/components/board-view.tsx` — update column drag handler to call backend command
- Backend: add `column.reorder` command (or extend `column.update`) that takes `{ id, target_index }` and computes new order values for all affected columns atomically

## Acceptance Criteria
- [ ] No column order calculation in the frontend
- [ ] Column drag-drop still works
- [ ] Backend computes and persists order values atomically
- [ ] All tests pass