---
assignees:
- claude-code
position_column: todo
position_ordinal: cd80
title: 'board-selector: replace EditableMarkdown with Field for board name editing'
---
## What

Board name editing uses EditableMarkdown directly with useFieldUpdate. Board is an entity with a `name` field — should use `<Field>`.

**BLOCKED**: Board entities are not in the EntityStore. `refresh.ts` only loads task, tag, and actor into `entitiesByType`. Board entities come from `parseBoardData()` separately. `useFieldValue("board", boardId, "name")` returns undefined because Field can't find board entities in the store.

Must first add board entities to the EntityStore (see refresh.ts hardcoded entity type list card).

### Files to modify
- `kanban-app/ui/src/components/board-selector.tsx` — replace EditableMarkdown with Field
- Remove direct useFieldUpdate import

## Acceptance Criteria
- [ ] Board name editing goes through Field
- [ ] No direct EditableMarkdown or updateField in board-selector

## Tests
- [ ] Zero type errors"