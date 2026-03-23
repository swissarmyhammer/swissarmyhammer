---
assignees:
- claude-code
depends_on:
- 01KMBE4FGC1WWF29PV5SEJG4VY
position_column: done
position_ordinal: ffffffffffff8580
title: 'Field: color-palette field type'
---
## What

Wire color-palette field type through Field. ColorPaletteEditor becomes a dumb picker — Field handles persistence.

### Files to modify
- `kanban-app/ui/src/components/fields/field.tsx` — color-palette case
- `kanban-app/ui/src/components/fields/editors/color-palette-editor.tsx` — remove self-save

## Acceptance Criteria
- [ ] color-palette rows in editor-save.test.tsx pass

## Tests
- [ ] `cd kanban-app/ui && npx vitest run src/components/fields/editors/editor-save.test.tsx` — color rows green