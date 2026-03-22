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

Merge `CellEditor` (grid) and `FieldDispatch`'s editing branch (inspector) into one `FieldEditor` component. Now that editors save themselves, the dispatch is identical. Also clean up tests that tested the old container-driven save wiring — they're testing dead code paths.

### Files to modify
- `kanban-app/ui/src/components/fields/field-editor.tsx` — new unified component
- `kanban-app/ui/src/components/cells/cell-editor.tsx` — delete or re-export
- `kanban-app/ui/src/components/entity-inspector.tsx` — use `FieldEditor`
- `kanban-app/ui/src/components/grid-view.tsx` — use `FieldEditor`
- `kanban-app/ui/src/components/data-table.tsx` — update `renderEditor` signature

### Tests to clean up
- Remove/update tests in `entity-inspector.test.tsx` that assert container calls `dispatch_command` with `entity.update_field` — that's the editor's job now
- Remove/update tests in `field-update-context.test.tsx` if they test wiring that no longer exists
- Audit any other test that mocks or asserts container-level `updateField` calls

### Approach
1. `FieldEditor` takes `field`, `entity`, `mode`, `onDone`, `onCancel`
2. Single switch on `resolveEditor(field)`, passes entity identity to each editor
3. Inspector and grid both render `<FieldEditor>`, differ only by `mode`
4. Delete `CellEditor` and `FieldDispatch` editing branch
5. Delete or update tests that assert the old container save path

## Acceptance Criteria
- [ ] One switch statement, not two
- [ ] Inspector and grid use same component
- [ ] No stale tests asserting container-level saves
- [ ] No behavioral change — all tests pass

## Tests
- [ ] `cd kanban-app/ui && npx vitest run` — full suite green, no dead test code