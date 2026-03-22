---
assignees:
- claude-code
depends_on:
- 01KMASVEQA7K7F1TKE3ACAWDXT
- 01KMASWCVYNTTX7GRKPPTCVQAX
position_column: todo
position_ordinal: '9e80'
title: MultiSelectEditor — call updateField directly
---
## What

Update `MultiSelectEditor` to use the new `EditorProps` contract.

### Files to modify
- `kanban-app/ui/src/components/fields/editors/multi-select-editor.tsx`

### Approach
1. Accept `entityType`, `entityId`, `fieldName` from props
2. Call `updateField` on commit (blur / popover close)
3. Replace `onCommit` with `onDone()`

## Acceptance Criteria
- [ ] Editor calls `updateField` directly
- [ ] Multi-select adapter in `editor-save.test.tsx` passes all keymap × exit combos

## Tests
- [ ] `cd kanban-app/ui && npx vitest run src/components/fields/editors/editor-save.test.tsx` — multi-select rows green