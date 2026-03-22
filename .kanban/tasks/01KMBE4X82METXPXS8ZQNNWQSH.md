---
assignees:
- claude-code
depends_on:
- 01KMBE4FGC1WWF29PV5SEJG4VY
position_column: todo
position_ordinal: b080
title: 'Field: markdown field type (compact + full)'
---
## What

Wire the markdown/text field type through Field. Compact mode uses FieldPlaceholderEditor (rename to TextEditor). Full mode uses EditableMarkdown. Both save through Field's updateField, not their own.

### Files to modify
- `kanban-app/ui/src/components/fields/field.tsx` — markdown case in dispatch
- `kanban-app/ui/src/components/fields/field-placeholder.tsx` — remove self-save logic, accept value + onChange, let Field handle persistence
- `kanban-app/ui/src/components/editable-markdown.tsx` — same: dumb editor, Field saves

## Acceptance Criteria
- [ ] markdown rows in editor-save.test.tsx pass (all keymaps × all exits × both modes)

## Tests
- [ ] `cd kanban-app/ui && npx vitest run src/components/fields/editors/editor-save.test.tsx` — markdown rows green