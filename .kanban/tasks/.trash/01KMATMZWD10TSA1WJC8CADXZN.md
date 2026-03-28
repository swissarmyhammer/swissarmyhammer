---
assignees:
- claude-code
depends_on:
- 01KMASVT0S8EHWVJ0MSPFCG5RY
position_column: todo
position_ordinal: b080
title: Rename FieldPlaceholderEditor to TextEditor
---
## What

`FieldPlaceholderEditor` is a misleading name — it's the actual CM6 text/markdown editor, not a placeholder. Rename to `TextEditor`.

### Files to modify
- `kanban-app/ui/src/components/fields/field-placeholder.tsx` — rename component, rename file to `text-editor.tsx`
- `kanban-app/ui/src/components/fields/editors/markdown-editor.tsx` — update import
- `kanban-app/ui/src/components/fields/editors/editor-save.test.tsx` — update import and adapter name
- Any other file importing `FieldPlaceholderEditor`

## Acceptance Criteria
- [ ] Component renamed to `TextEditor`
- [ ] File renamed to `text-editor.tsx`
- [ ] All imports updated
- [ ] `cd kanban-app/ui && npx vitest run` — full suite green

## Tests
- [ ] Full suite green