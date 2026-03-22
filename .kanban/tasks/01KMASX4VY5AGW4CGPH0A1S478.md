---
assignees:
- claude-code
depends_on:
- 01KMASVEQA7K7F1TKE3ACAWDXT
- 01KMASWCVYNTTX7GRKPPTCVQAX
position_column: todo
position_ordinal: 9a80
title: NumberEditor — call updateField directly
---
## What

Update `NumberEditor` to use the new `EditorProps` contract. It calls `useFieldUpdate().updateField()` itself on blur and Enter.

### Files to modify
- `kanban-app/ui/src/components/fields/editors/number-editor.tsx`

### Approach
1. Accept `entityType`, `entityId`, `fieldName` from props
2. Call `updateField` on blur and Enter
3. Replace `onCommit` calls with `onDone()`
4. Escape calls `onCancel()` (no save) for CUA/emacs; saves then `onDone()` for vim

## Acceptance Criteria
- [ ] Editor calls `updateField` directly
- [ ] Number adapter in `editor-save.test.tsx` passes all keymap × exit combos

## Tests
- [ ] `cd kanban-app/ui && npx vitest run src/components/fields/editors/editor-save.test.tsx` — number rows green