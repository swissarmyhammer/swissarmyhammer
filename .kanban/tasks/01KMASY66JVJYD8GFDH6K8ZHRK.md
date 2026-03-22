---
assignees:
- claude-code
depends_on:
- 01KMASVEQA7K7F1TKE3ACAWDXT
- 01KMASWCVYNTTX7GRKPPTCVQAX
position_column: done
position_ordinal: fffffffffff980
title: MultiSelectEditor — call updateField directly
---
## What

Update `MultiSelectEditor` to use the new `EditorProps` contract, and add its adapter to the test matrix.

### Files to modify
- `kanban-app/ui/src/components/fields/editors/multi-select-editor.tsx`
- `kanban-app/ui/src/components/fields/editors/editor-save.test.tsx` — add multi-select adapter (needs entity store + schema providers)

### Approach
1. Accept `entityType`, `entityId`, `fieldName` from props
2. Call `updateField` on commit (blur / popover close)
3. Replace `onCommit` with `onDone()`
4. Add multi-select adapter to test matrix with required providers

## Acceptance Criteria
- [ ] Editor calls `updateField` directly
- [ ] Multi-select adapter in test matrix
- [ ] Multi-select rows in `editor-save.test.tsx` pass all keymap × exit combos

## Tests
- [ ] `cd kanban-app/ui && npx vitest run src/components/fields/editors/editor-save.test.tsx` — multi-select rows green