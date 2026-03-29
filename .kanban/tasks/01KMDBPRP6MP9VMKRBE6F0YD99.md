---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffff8980
title: 'field-renderer: delete — replaced by Field'
---
## What

`field-renderer.tsx` has its own dispatch + useFieldUpdate. It's an older version of what Field now does. Delete it and update any imports to use Field.

### Files to modify
- `kanban-app/ui/src/components/field-renderer.tsx` — delete
- Any file importing it — switch to Field

## Acceptance Criteria
- [ ] field-renderer.tsx deleted
- [ ] Zero type errors

## Tests
- [ ] Zero type errors