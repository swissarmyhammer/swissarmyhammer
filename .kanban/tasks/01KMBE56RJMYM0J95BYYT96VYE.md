---
assignees:
- claude-code
depends_on:
- 01KMBE4FGC1WWF29PV5SEJG4VY
position_column: done
position_ordinal: ffffffffffffcf80
title: 'Field: number field type'
---
## What

Wire number field type through Field. NumberEditor becomes a dumb input — Field handles persistence.

### Files to modify
- `kanban-app/ui/src/components/fields/field.tsx` — number case
- `kanban-app/ui/src/components/fields/editors/number-editor.tsx` — remove self-save, accept value + onChange

## Acceptance Criteria
- [ ] number rows in editor-save.test.tsx pass

## Tests
- [ ] `cd kanban-app/ui && npx vitest run src/components/fields/editors/editor-save.test.tsx` — number rows green