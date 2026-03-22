---
assignees:
- claude-code
depends_on:
- 01KMBE4FGC1WWF29PV5SEJG4VY
position_column: done
position_ordinal: fffffffffffe80
title: Wire Field into board cards, inspector, and grid — all field rendering goes through Field
---
## What

Replace every place that renders a field display or editor with `<Field>`. After this card, Field is the only way to render a field. Everything will be broken until the per-field-type cards fix each editor/display.

### Files to modify
- `kanban-app/ui/src/components/entity-inspector.tsx` — FieldRow renders `<Field mode="full">` instead of FieldDispatch
- `kanban-app/ui/src/components/grid-view.tsx` — renderEditor uses `<Field mode="compact">` instead of CellEditor
- `kanban-app/ui/src/components/data-table.tsx` — cell rendering uses `<Field mode="compact">` for display too
- `kanban-app/ui/src/components/entity-card.tsx` — CardFieldDispatch uses `<Field mode="compact">`

### Files to delete
- `kanban-app/ui/src/components/cells/cell-editor.tsx` — replaced by Field
- Inspector's FieldDispatch function — replaced by Field

## Acceptance Criteria
- [ ] Inspector renders `<Field mode="full">` for every field row
- [ ] Grid renders `<Field mode="compact">` for display and editing
- [ ] Entity cards render `<Field mode="compact">` for card fields
- [ ] CellEditor deleted
- [ ] FieldDispatch deleted
- [ ] App compiles (may render nothing — Field is a skeleton)

## Tests
- [ ] `cd kanban-app/ui && npx vitest run` — existing tests may break (expected, Field is skeleton)