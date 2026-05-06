---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffcb80
title: Update editor-save.test.tsx to test through Field component, not individual editors
---
## What

The current test mocks `useFieldUpdate` at the module level and tests individual editors directly. This hides real bugs. Rewrite the test to render a `Field` component (to be created in the next card) that is a fully data-bound control:

- **Reads** its current value from the entity store (via `useEntityStore`)
- **Writes** via `updateField` on every save-worthy exit path
- **Stays in sync** — re-renders when the entity store updates

The test renders `<Field entity={...} fieldDef={...} mode={...} onDone={...} />` inside real providers (`FieldUpdateProvider`, `EntityStoreProvider`). No module-level mock of `useFieldUpdate`.

### Files to modify
- `kanban-app/ui/src/components/fields/editors/editor-save.test.tsx` — rewrite

### Approach
1. Remove `vi.mock("@/lib/field-update-context")`
2. Wrap in real `FieldUpdateProvider` + `EntityStoreProvider`
3. Mock only `invoke` — assert `invoke("dispatch_command", { cmd: "entity.update_field", ... })` is called
4. Keep the matrix: all field types × all keymaps × all exit paths × both modes
5. Add a sync test: update entity store after save, verify Field re-renders with new value
6. Tests will fail (Field doesn't exist yet) — that's the red

## Acceptance Criteria
- [ ] Tests render through Field, not individual editors
- [ ] No module-level mock of useFieldUpdate — real context exercised
- [ ] Tests assert on the actual invoke call
- [ ] Includes a data-sync test (entity store update → Field re-renders)
- [ ] All tests fail (Field component doesn't exist yet)

## Tests
- [ ] `cd kanban-app/ui && npx vitest run src/components/fields/editors/editor-save.test.tsx` — runs, all fail