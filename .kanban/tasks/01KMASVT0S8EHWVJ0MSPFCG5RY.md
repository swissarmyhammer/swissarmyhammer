---
assignees:
- claude-code
depends_on:
- 01KMASVEQA7K7F1TKE3ACAWDXT
position_column: todo
position_ordinal: '9880'
title: FieldPlaceholderEditor — call updateField directly
---
## What

Update `FieldPlaceholderEditor` to use the new `EditorProps` contract. It calls `useFieldUpdate().updateField()` itself on blur, Enter, and vim Escape. Reports `onDone()` to container after saving.

### Files to modify
- `kanban-app/ui/src/components/fields/field-placeholder.tsx` — `FieldPlaceholderEditor`

### Approach
1. Accept `entityType`, `entityId`, `fieldName` from props
2. Call `useFieldUpdate().updateField(entityType, entityId, fieldName, text)` on every save path
3. Call `onDone()` after save succeeds
4. `onCancel()` does NOT save (CUA Escape = discard)
5. Vim Escape always saves then calls `onDone()` — regardless of `onSubmit`

## Acceptance Criteria
- [ ] Editor calls `updateField` directly — no container provides save logic
- [ ] Vim Escape saves in both compact and full mode
- [ ] CUA/emacs Escape discards in both modes
- [ ] Blur always saves

## Tests
- [ ] Markdown adapter in `editor-save.test.tsx` passes for all keymap × exit combinations
- [ ] `cd kanban-app/ui && npx vitest run src/components/fields/editors/editor-save.test.tsx` — markdown rows green