---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffd680
title: Remove self-save updateField calls from all editors — Field handles persistence
---
## What

The half-migration left updateField calls inside individual editors. Now that Field owns persistence via handleCommit, editors should not call updateField themselves. Remove the self-save logic from:

- `field-placeholder.tsx` — remove useFieldUpdate, saveToEntity, saveToEntityRef
- `number-editor.tsx` — remove useFieldUpdate, direct updateField call
- `date-editor.tsx` — remove useFieldUpdate, direct updateField call
- `multi-select-editor.tsx` — remove useFieldUpdate, direct updateField call
- `select-editor.tsx` — already clean (uses onCommit only)
- `color-palette-editor.tsx` — already clean (uses onCommit only)

Also remove entityType/entityId/fieldName props from editors that still accept them — Field passes value and onCommit, not entity identity.

## Acceptance Criteria
- [ ] Zero useFieldUpdate imports in any editor
- [ ] Zero updateField calls in any editor
- [ ] Editors only call onCommit/onCancel
- [ ] Zero type errors

## Tests
- [ ] `cd kanban-app/ui && npx vitest run` — no regressions