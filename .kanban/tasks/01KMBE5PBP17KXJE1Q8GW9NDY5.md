---
assignees:
- claude-code
depends_on:
- 01KMBE4FGC1WWF29PV5SEJG4VY
position_column: done
position_ordinal: ffffffffffff8480
title: 'Field: date field type'
---
## What

Wire date field type through Field. DateEditor becomes a dumb picker — Field handles persistence.

### Files to modify
- `kanban-app/ui/src/components/fields/field.tsx` — date case
- `kanban-app/ui/src/components/fields/editors/date-editor.tsx` — remove self-save

## Acceptance Criteria
- [ ] date rows in editor-save.test.tsx pass

## Tests
- [ ] `cd kanban-app/ui && npx vitest run src/components/fields/editors/editor-save.test.tsx` — date rows green