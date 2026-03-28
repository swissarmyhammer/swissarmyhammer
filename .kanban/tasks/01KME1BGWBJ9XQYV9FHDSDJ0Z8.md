---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffff9580
title: 'multi-select-editor.test.tsx: rewrite tests for inline doc-as-truth model'
---
## What

Tests assert on the deleted external pill div (pills above CM6, × remove buttons). The editor was rewritten to use inline CM6 decorations — the doc IS the selection. Tests need full rewrite.

### Failures
- shows existing selections as pills
- actor selections render with Avatar component
- remove button removes item from selection

### What to test
- Initial doc contains prefixed tokens for existing selections (e.g. `#bug #feature `)
- Tokens are decorated as pills via CM6 marks
- Backspace in doc removes token text (and thus the selection)
- Commit parses doc text and resolves IDs
- Tags: unknown slugs auto-created. Actors/tasks: only resolved IDs kept.

## Acceptance Criteria
- [ ] All multi-select-editor tests pass
- [ ] Tests cover inline decoration model, not external pill div

## Tests
- [ ] `cd kanban-app/ui && npx vitest run src/components/fields/editors/multi-select-editor.test.tsx`"