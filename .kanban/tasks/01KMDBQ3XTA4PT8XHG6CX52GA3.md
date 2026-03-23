---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffff8f80
title: 'board-view: remove direct updateField for drag position updates'
---
## What

`board-view.tsx` calls useFieldUpdate directly for drag-and-drop position updates (position_column, position_ordinal, position_swimlane). These are field mutations that bypass Field.

### Open question
Drag-and-drop updates multiple fields atomically (column + ordinal + swimlane). Field updates one field at a time. This may need a batch update mechanism or a different approach. Investigate before fixing.

### Files to modify
- `kanban-app/ui/src/components/board-view.tsx` — remove direct updateField

## Acceptance Criteria
- [ ] No direct updateField in board-view
- [ ] Drag-and-drop still works

## Tests
- [ ] Zero type errors