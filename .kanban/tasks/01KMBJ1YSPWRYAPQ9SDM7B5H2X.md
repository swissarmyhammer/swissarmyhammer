---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffff8a80
title: Rename FieldPlaceholderEditor to TextEditor
---
## What

`FieldPlaceholderEditor` is a misleading name — it's the actual CM6 text/markdown editor, not a placeholder. Rename to `TextEditor`.

### Files to modify
- `kanban-app/ui/src/components/fields/field-placeholder.tsx` — rename component, rename file to `text-editor.tsx`
- `kanban-app/ui/src/components/fields/registrations/markdown.tsx` — update import
- Any other file importing `FieldPlaceholderEditor`

## Acceptance Criteria
- [ ] Component renamed to `TextEditor`
- [ ] File renamed to `text-editor.tsx`
- [ ] All imports updated
- [ ] `npx tsc --noEmit` clean
- [ ] `cd kanban-app/ui && npx vitest run` — no regressions

## Tests
- [ ] Zero type errors
- [ ] All previously passing tests still pass