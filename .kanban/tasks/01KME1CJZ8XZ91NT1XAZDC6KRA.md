---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffe680
title: 'editor-save.test.tsx: fix multi-select Escape tests and entity coverage gaps'
---
## What

20 multi-select test failures + 2 entity coverage failures in the editor-save matrix test.

### Multi-select Escape failures (16)
All multi-select fields (tags, assignees, depends_on, attachments) × CUA/emacs × Escape. The test fires Escape expecting no commit (cancel behavior), but multi-select's Escape handler calls commit. This is correct behavior — multi-select commits on both Enter and Escape (saves whatever is in the doc). The test expectation is wrong.

### Multi-select blur/attachments failures (4)
Attachments in full/emacs mode — blur and Escape. jsdom can't model the CM6 blur/focus interaction properly.

### Entity coverage failures (2)
position_column and position_swimlane fields use `editor: select` but no select options are available in the test schema, so Field renders null. Need to either provide mock options or exclude position fields from coverage.

## Acceptance Criteria
- [ ] Multi-select Escape tests: update expectations — Escape commits (not cancels) for multi-select
- [ ] Entity coverage: fix or skip position_column/position_swimlane
- [ ] Zero failures in editor-save.test.tsx

## Tests
- [ ] `cd kanban-app/ui && npx vitest run src/components/fields/editors/editor-save.test.tsx`"