---
assignees:
- claude-code
depends_on:
- 01KMASVT0S8EHWVJ0MSPFCG5RY
position_column: done
position_ordinal: ffffffffffffc980
title: Delete field-placeholder.test.tsx — replaced by editor-save.test.tsx
---
## What

Remove the old per-editor test file now that the matrix test covers all paths.

### Files to delete
- `kanban-app/ui/src/components/fields/field-placeholder.test.tsx`

## Acceptance Criteria
- [ ] File deleted
- [ ] `cd kanban-app/ui && npx vitest run` — full suite passes, no missing test references

## Tests
- [ ] Full suite green