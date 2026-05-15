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
position_column: done
position_ordinal: ffffffffffffca80
title: Remove container save logic from grid-view and entity-inspector
---
## What

Now that editors call `updateField` themselves, remove the save logic from the containers. `onCommit`/`handleCommit` in grid-view's `renderEditor` and inspector's `FieldRow` currently call `updateField` — that must stop. They should only pass lifecycle callbacks (`onDone`, `onCancel`). Also remove tests that assert the old container-driven save wiring.

### Files to modify
- `kanban-app/ui/src/components/grid-view.tsx` — `renderEditor` stops calling `updateField`, passes `onDone`/`onCancel` only
- `kanban-app/ui/src/components/entity-inspector.tsx` — `FieldRow.handleCommit` stops calling `updateField`, becomes just `setEditing(false)`

### Tests to clean up
- `entity-inspector.test.tsx` — remove/update tests asserting container calls `dispatch_command` with `entity.update_field`
- `field-update-context.test.tsx` — review; the hook still exists but containers no longer use it directly
- Any other test asserting container-level `updateField` calls

### Approach
1. grid-view `handleCommit`: remove `updateField(...)` call, keep `grid.exitEdit()`
2. inspector `handleCommit`: remove `updateField(...)` call, keep `setEditing(false)`
3. Both now just signal "editing is done" — the editor already saved
4. Delete or update tests that assert the old save path

## Acceptance Criteria
- [ ] grid-view `renderEditor` does not call `updateField`
- [ ] inspector `FieldRow` does not call `updateField`
- [ ] No double-saves — `updateField` called exactly once per edit (by the editor)
- [ ] `.catch(() => {})` silent error swallowing removed from containers
- [ ] No stale tests asserting container-level saves

## Tests
- [ ] `cd kanban-app/ui && npx vitest run` — full suite green, no dead test code
- [ ] Manual: edit in grid, edit in inspector — both save correctly, no double writes in Rust logs