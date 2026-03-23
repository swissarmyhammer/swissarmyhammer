---
assignees:
- claude-code
position_column: todo
position_ordinal: c980
title: 'quick-capture: replace FieldPlaceholderEditor with Field for task title input'
---
## What

Quick capture uses FieldPlaceholderEditor directly for task title input. This is a field editor bypass.

### Files to modify
- `kanban-app/ui/src/components/quick-capture.tsx` — replace with Field or rethink (quick capture creates a new entity, Field binds to an existing one — may need a different approach)

## Acceptance Criteria
- [ ] No direct FieldPlaceholderEditor import in quick-capture
- [ ] Quick capture still works

## Tests
- [ ] Zero type errors