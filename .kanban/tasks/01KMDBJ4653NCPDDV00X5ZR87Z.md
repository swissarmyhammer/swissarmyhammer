---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffd980
title: 'column-view: replace EditableMarkdown with Field for column name editing'
---
## What

Column header name editing uses EditableMarkdown directly with an onRenameColumn callback. This is editing the column entity's `name` field — should use `<Field>`.

### Files to modify
- `kanban-app/ui/src/components/column-view.tsx` — replace EditableMarkdown with `<Field entityType="column" entityId={column.id} fieldDef={...} mode="compact" />`
- Remove `onRenameColumn` prop — Field handles persistence

## Acceptance Criteria
- [ ] Column name editing goes through Field
- [ ] No direct EditableMarkdown import in column-view

## Tests
- [ ] Zero type errors
- [ ] Column name editing still works in the app