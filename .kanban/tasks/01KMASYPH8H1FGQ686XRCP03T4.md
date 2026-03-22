---
assignees:
- claude-code
depends_on:
- 01KMASVT0S8EHWVJ0MSPFCG5RY
- 01KMASX4VY5AGW4CGPH0A1S478
- 01KMASXDVH5WJEP0BAX0D20N6C
- 01KMASXNTBZRKYNSBPE1BQ82QC
- 01KMASXXREC3EN3HGKRAHNKJEN
- 01KMASY66JVJYD8GFDH6K8ZHRK
position_column: todo
position_ordinal: 9f80
title: Eliminate duplicate editor dispatch — single FieldEditor component
---
## What

Merge `CellEditor` (grid) and `FieldDispatch`'s editing branch (inspector) into one `FieldEditor` component. Now that editors save themselves, the dispatch is identical.

### Files to modify
- `kanban-app/ui/src/components/fields/field-editor.tsx` — new unified component
- `kanban-app/ui/src/components/cells/cell-editor.tsx` — delete or re-export
- `kanban-app/ui/src/components/entity-inspector.tsx` — use `FieldEditor`
- `kanban-app/ui/src/components/grid-view.tsx` — use `FieldEditor`
- `kanban-app/ui/src/components/data-table.tsx` — update `renderEditor` signature

### Approach
1. `FieldEditor` takes `field`, `entity`, `mode`, `onDone`, `onCancel`
2. Single switch on `resolveEditor(field)`, passes entity identity to each editor
3. Inspector and grid both render `<FieldEditor>`, differ only by `mode`
4. Delete `CellEditor` and `FieldDispatch` editing branch

## Acceptance Criteria
- [ ] One switch statement, not two
- [ ] Inspector and grid use same component
- [ ] No behavioral change — all tests pass

## Tests
- [ ] `cd kanban-app/ui && npx vitest run` — full suite green