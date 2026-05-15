---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffd180
title: 'SelectEditor: replace HTML select with shadcn Select, handle Enter to commit'
---
## What

SelectEditor uses a raw HTML `<select>` element. Should use a shadcn Select (or Combobox) component for visual consistency and proper keyboard handling. Enter should commit the current selection — currently it does nothing because HTML select doesn't respond to Enter.

### Files to modify
- `kanban-app/ui/src/components/fields/editors/select-editor.tsx` — replace with shadcn component

## Acceptance Criteria
- [ ] Uses shadcn Select component, not HTML `<select>`
- [ ] Enter commits the current selection
- [ ] Escape cancels (CUA/emacs) or commits (vim)
- [ ] select Enter rows in editor-save.test.tsx pass
- [ ] Visual consistency with rest of UI

## Tests
- [ ] `cd kanban-app/ui && npx vitest run src/components/fields/editors/editor-save.test.tsx` — all select rows green including Enter