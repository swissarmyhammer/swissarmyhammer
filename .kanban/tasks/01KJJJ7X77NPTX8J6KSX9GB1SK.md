---
title: Replace all EditableText usages with EditableMarkdown
position:
  column: done
  ordinal: b8
---
Swap `EditableText` for `EditableMarkdown` in every location it's used, then delete the old component.

**Current usages of EditableText:**
1. `task-detail-panel.tsx` — task title (single-line) and description (multiline)
2. `column-view.tsx` — column name (single-line)

**Changes per file:**

**`task-detail-panel.tsx`:**
- Import `EditableMarkdown` instead of `EditableText`
- Title: `<EditableMarkdown value={task.title} onCommit={...} className="..." inputClassName="..." />`
- Description: `<EditableMarkdown value={task.description ?? ""} onCommit={...} multiline placeholder="Add description..." />`
- Fix Escape key handler: check for `.cm-focused` ancestor in addition to INPUT/TEXTAREA tags

**`column-view.tsx`:**
- Import `EditableMarkdown` instead of `EditableText`
- Column name: `<EditableMarkdown value={column.name} onCommit={...} className="..." inputClassName="..." />`

**Cleanup:**
- Delete `ui/src/components/editable-text.tsx`
- Delete `ui/src/components/editable-text.test.tsx`

**Files:**
- `ui/src/components/task-detail-panel.tsx` (modify)
- `ui/src/components/column-view.tsx` (modify)
- `ui/src/components/editable-text.tsx` (delete)
- `ui/src/components/editable-text.test.tsx` (delete)

**Verify:**
- `npm run build` passes with no references to EditableText
- `npm run test` passes
- All inline editing works: column names, task titles, descriptions

## Checklist
- [ ] Replace EditableText with EditableMarkdown in task-detail-panel.tsx
- [ ] Replace EditableText with EditableMarkdown in column-view.tsx
- [ ] Fix Escape handler for CodeMirror focus
- [ ] Delete editable-text.tsx and editable-text.test.tsx
- [ ] Verify build and tests pass
- [ ] Manual test all edit flows in running app