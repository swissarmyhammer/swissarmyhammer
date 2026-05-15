---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffda80
title: 'entity-inspector.tsx: FieldDispatch uses anonymous inline prop type'
---
**File:** `kanban-app/ui/src/components/entity-inspector.tsx:193`\n\n`FieldDispatch` has 7 props defined as `}: { field, value, entity, editing, onEdit, onCommit, onCancel }` inline. Extract to `interface FieldDispatchProps`. #props-slop