---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffe480
title: 'entity-card.test.tsx: update tests for Field-based card rendering'
---
## What

Tests were written against the old entity-card (pre-Field, hardcoded CardFieldDispatch). The card now uses `<Field>` with schema-driven rendering, per-field editing state, and inline icons. Tests need updating.

### Failures (6)
- renders title as markdown (bold text)
- enters edit mode when title is clicked
- saving edited title calls invoke with correct camelCase params
- shows progress bar when description has checkboxes
- shows 0% progress when no checkboxes are checked
- shows 100% progress when all checkboxes are checked

### What to update
- Tests need real providers (SchemaProvider, EntityStoreProvider, FieldUpdateProvider)
- Progress tests: value is now a computed `{ total, completed, percent }` object via ProgressDisplay, not SubtaskProgress
- Edit tests: Field + editing state, not direct CM6 rendering

## Acceptance Criteria
- [ ] All 6 entity-card tests pass
- [ ] Tests use real providers, not mocked hooks

## Tests
- [ ] `cd kanban-app/ui && npx vitest run src/components/entity-card.test.tsx`"