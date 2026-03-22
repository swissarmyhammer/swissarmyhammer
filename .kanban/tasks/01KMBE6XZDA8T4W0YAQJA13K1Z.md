---
assignees:
- claude-code
depends_on:
- 01KMBE4FGC1WWF29PV5SEJG4VY
position_column: todo
position_ordinal: b580
title: 'Field: multi-select field type'
---
## What

Wire multi-select field type through Field. MultiSelectEditor becomes a dumb selector — Field handles persistence. Pill × removal saves immediately through Field.

### Files to modify
- `kanban-app/ui/src/components/fields/field.tsx` — multi-select case
- `kanban-app/ui/src/components/fields/editors/multi-select-editor.tsx` — remove self-save

## Acceptance Criteria
- [ ] multi-select rows in editor-save.test.tsx pass
- [ ] Pill × removal saves immediately

## Tests
- [ ] `cd kanban-app/ui && npx vitest run src/components/fields/editors/editor-save.test.tsx` — multi-select rows green