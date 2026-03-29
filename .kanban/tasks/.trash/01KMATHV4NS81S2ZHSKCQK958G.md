---
assignees:
- claude-code
depends_on:
- 01KMATAV6083ZASDDZ688NA212
position_column: todo
position_ordinal: af80
title: Remove legacy onCommit from EditorProps
---
## What

Once all editors call `updateField` directly, remove the legacy `onCommit` field from `EditorProps` and make `entityType`, `entityId`, `fieldName` required. Clean up any remaining references.

### Files to modify
- `kanban-app/ui/src/components/fields/editors/markdown-editor.tsx` — remove `onCommit` from `EditorProps`, make entity identity fields required
- Any editor still referencing `onCommit` — remove those references

## Acceptance Criteria
- [ ] `onCommit` removed from `EditorProps`
- [ ] `entityType`, `entityId`, `fieldName` are required (not optional)
- [ ] No editor references `onCommit`
- [ ] `cd kanban-app/ui && npx vitest run` — full suite green

## Tests
- [ ] Full suite green — no code references the removed field