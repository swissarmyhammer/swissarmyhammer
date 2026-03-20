---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffff780
title: Remove grey opacity on blocked cards
---
## What
Cards with unmet dependencies render at 50% opacity (`opacity-50`), making them look broken rather than informative. Remove this visual treatment entirely.

**Files:**
- `kanban-app/ui/src/components/entity-card.tsx` — remove `isBlocked` from props interface (line 17), remove from destructuring (line 32), remove `isBlocked ? "opacity-50" : ""` from className (line 75)
- `kanban-app/ui/src/components/sortable-task-card.tsx` — remove `isBlocked` from `DraggableTaskCardProps` (line 7), remove from destructuring (line 21), remove `isBlocked={isBlocked}` passed to EntityCard (line 71)
- `kanban-app/ui/src/components/column-view.tsx` — remove `isBlocked={entity.fields.ready === false}` from DraggableTaskCard (line 203)

## Acceptance Criteria
- [ ] No card ever renders at reduced opacity due to dependency status
- [ ] `isBlocked` prop removed from EntityCard and DraggableTaskCard interfaces
- [ ] No references to `isBlocked` remain in the card rendering path

## Tests
- [ ] `npm run build` in ui/ succeeds (no TypeScript errors from removed prop)
- [ ] Manual: cards with unmet dependencies render at full opacity on the board