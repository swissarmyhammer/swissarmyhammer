---
assignees:
- claude-code
depends_on:
- 01KMBE3DQN58BKFCJGRK40GPYA
position_column: todo
position_ordinal: af80
title: Field component — data-bound control that owns read, write, and sync
---
## What

Create a `Field` component that is the single way to render any field in the app. It is a live data-bound control:

- **Reads** its current value from `useEntityStore().getEntity(entityType, entityId).fields[fieldName]`
- **Writes** via `useFieldUpdate().updateField()` — the Field owns saving, not the container
- **Syncs** — re-renders when the entity store updates (entity-field-changed event)
- **Dispatches** display and editor components based on field YAML config (`resolveEditor`, `field.display`)
- **Accepts** `mode="compact"|"full"` to control presentation
- **Signals** `onDone()` / `onCancel()` to container for lifecycle (close editor, exit edit mode)

Inspector renders `<Field>`. Grid renders `<Field>`. No other component touches editors or displays directly.

### Files to create
- `kanban-app/ui/src/components/fields/field.tsx`

### Files to delete/gut
- `kanban-app/ui/src/components/cells/cell-editor.tsx` — replaced by Field
- `kanban-app/ui/src/components/entity-inspector.tsx` — FieldDispatch replaced by Field
- `kanban-app/ui/src/components/fields/editors/markdown-editor.tsx` — MarkdownEditor wrapper replaced by Field dispatch

### Props
```tsx
interface FieldProps {
  entityType: string;
  entityId: string;
  fieldDef: FieldDef;
  mode: "compact" | "full";
  editing: boolean;
  onEdit?: () => void;
  onDone?: () => void;
  onCancel?: () => void;
}
```

## Acceptance Criteria
- [ ] Field reads value from entity store, not from props
- [ ] Field writes via updateField on all save paths
- [ ] Field re-renders when entity store updates
- [ ] Field dispatches to correct display/editor based on YAML config
- [ ] Inspector and grid both use Field — no other component renders editors
- [ ] CellEditor and FieldDispatch editing branch deleted

## Tests
- [ ] editor-save.test.tsx matrix passes through Field
- [ ] `cd kanban-app/ui && npx vitest run` — full suite green