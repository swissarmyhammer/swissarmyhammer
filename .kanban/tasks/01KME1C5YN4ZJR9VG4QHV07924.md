---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffff9780
title: 'entity-inspector.test.tsx: update tests for Field-based inspector rendering'
---
## What

Tests have stale assertions from pre-Field architecture. The inspector now renders all fields via `<Field>` with schema-driven dispatch.

### Failures (3)
- renders markdown fields with EditableMarkdown (click enters edit mode)
- allows editing computed tag fields via multi-select
- body_field renders #tag as a styled pill when tag entity exists

### What to update
- Edit mode test: Field handles editing, not direct EditableMarkdown
- Multi-select test: editor is now inline doc-as-truth, not external pill div
- Tag pill test: may need updated selectors for CM6 decoration-based pills

## Acceptance Criteria
- [ ] All 3 entity-inspector tests pass

## Tests
- [ ] `cd kanban-app/ui && npx vitest run src/components/entity-inspector.test.tsx`"