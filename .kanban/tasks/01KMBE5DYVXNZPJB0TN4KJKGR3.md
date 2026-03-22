---
assignees:
- claude-code
depends_on:
- 01KMBE4FGC1WWF29PV5SEJG4VY
position_column: todo
position_ordinal: b280
title: 'Field: select field type'
---
## What

Wire select field type through Field. SelectEditor becomes a dumb dropdown — Field handles persistence.

### Files to modify
- `kanban-app/ui/src/components/fields/field.tsx` — select case
- `kanban-app/ui/src/components/fields/editors/select-editor.tsx` — remove self-save

## Acceptance Criteria
- [ ] select rows in editor-save.test.tsx pass

## Tests
- [ ] `cd kanban-app/ui && npx vitest run src/components/fields/editors/editor-save.test.tsx` — select rows green