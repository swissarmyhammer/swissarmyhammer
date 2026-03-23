---
assignees:
- claude-code
position_column: todo
position_ordinal: c880
title: 'board-selector: replace EditableMarkdown with Field for board name editing'
---
## What

Board name editing uses EditableMarkdown directly with useFieldUpdate. Board is an entity with a `name` field — should use `<Field>`.

### Files to modify
- `kanban-app/ui/src/components/board-selector.tsx` — replace EditableMarkdown with Field
- Remove direct useFieldUpdate import

## Acceptance Criteria
- [ ] Board name editing goes through Field
- [ ] No direct EditableMarkdown or updateField in board-selector

## Tests
- [ ] Zero type errors