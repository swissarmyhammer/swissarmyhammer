---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: '8e80'
title: Eliminate duplicate editor dispatch — single FieldEditor component
---
## What

`CellEditor` (grid) and `FieldDispatch` (inspector) both have the same switch statement dispatching to editors by `resolveEditor(field)`. Now that editors own their own save, the dispatch logic is identical — merge into one `FieldEditor` component that both grid and inspector use.

### Files to modify
- `kanban-app/ui/src/components/fields/field-editor.tsx` — new unified component (rename/refactor from cell-editor.tsx)
- `kanban-app/ui/src/components/cells/cell-editor.tsx` — delete or reduce to re-export
- `kanban-app/ui/src/components/entity-inspector.tsx` — replace `FieldDispatch` editing branch with `FieldEditor`
- `kanban-app/ui/src/components/grid-view.tsx` — use `FieldEditor` instead of `CellEditor`
- `kanban-app/ui/src/components/data-table.tsx` — update `renderEditor` signature

### Approach
1. Create `FieldEditor` that takes `field`, `entity`, `mode`, `onDone`, `onCancel`
2. Contains the single switch on `resolveEditor(field)`
3. Passes `entityType`, `entityId`, `fieldName` to each editor (editors save themselves)
4. Inspector and grid both render `<FieldEditor>` — difference is only `mode="full"` vs `mode="compact"`
5. Delete `CellEditor` and the editing branch of `FieldDispatch`

## Acceptance Criteria
- [ ] One switch statement dispatching to editors, not two
- [ ] `CellEditor` deleted or reduced to alias
- [ ] Inspector editing branch in `FieldDispatch` replaced with `FieldEditor`
- [ ] Grid uses same `FieldEditor` component
- [ ] No behavioral change — all 419+ tests pass

## Tests
- [ ] Existing `entity-inspector.test.tsx` passes unchanged
- [ ] Existing `field-placeholder.test.tsx` passes unchanged  
- [ ] `cd kanban-app/ui && npx vitest run` — all tests pass