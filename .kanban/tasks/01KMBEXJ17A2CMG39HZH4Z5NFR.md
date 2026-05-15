---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffd580
title: 'Audit: find all Field bypasses — places that render editors/displays directly'
---
## What

Find and eliminate every place that renders editors or displays directly instead of going through Field. Each bypass is a bug — fix it in this card or split into a new card if substantial.

### What to search for and fix
- Imports of individual editors: MarkdownEditor, NumberEditor, SelectEditor, DateEditor, ColorPaletteEditor, MultiSelectEditor
- Imports of FieldPlaceholderEditor / EditableMarkdown used for field editing
- Imports of CellDispatch, CellEditor, FieldDispatch
- Direct calls to `updateField` that should go through Field
- Any component that reads `field.editor` or `field.display` to dispatch rendering (only Field should do this)

### Each bypass found gets fixed or carded — no exceptions.

## Acceptance Criteria
- [ ] Zero imports of individual editors outside of Field
- [ ] Zero imports of CellEditor, CellDispatch, FieldDispatch outside of Field
- [ ] Zero direct updateField calls outside of Field
- [ ] Only Field reads field.editor / field.display to dispatch rendering
- [ ] `cd kanban-app/ui && npx vitest run` — full suite green

## Tests
- [ ] Full suite green — no bypasses remain