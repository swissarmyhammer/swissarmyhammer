---
assignees:
- claude-code
depends_on:
- 01KNFJKD5E13JBPA3VRTKKKF2X
position_column: done
position_ordinal: ffffffffffffffffffffd180
title: 'Smoke test: grouped board end-to-end with drag across groups'
---
## What

Manual and automated end-to-end verification that the grouped board works correctly: group sections render, collapse/expand, drag within a group moves column/ordinal only, drag across groups updates the group field, and ungrouped mode is unchanged.

### Files to modify

1. **`kanban-app/ui/src/components/grouped-board-view.test.tsx`** — add integration-level tests:
   - Render with groupField active, verify group section headers appear
   - Simulate a drop across groups, verify both `task.move` and `entity.update_field` are dispatched
   - Simulate a drop within same group, verify only `task.move` is dispatched
   - Render without groupField, verify flat BoardView behavior

2. **`kanban-app/ui/src/components/board-view.test.tsx`** (if exists) — add test:
   - BoardView with `groupValue` prop passes it through to drop zone descriptors

### Manual smoke test checklist

- [ ] Open board with no grouping — looks exactly like before
- [ ] Select \"Group by: tags\" in perspective — sections appear
- [ ] Each section has correct label and task count
- [ ] Collapse a section — cards hide, header remains
- [ ] Expand a section — cards reappear
- [ ] Drag a card within same group, same column — ordinal updates
- [ ] Drag a card within same group, different column — column updates
- [ ] Drag a card from \"bug\" group to \"feature\" group — task's tags field updates (bug removed, feature added)
- [ ] Drag a card from a group to \"(ungrouped)\" — group value removed
- [ ] Drag a card from \"(ungrouped)\" to a group — group value added
- [ ] Group by single-value field (project) — verify simpler set semantics
- [ ] Remove grouping — board returns to flat view, no visual artifacts

## Acceptance Criteria

- [ ] All automated tests pass
- [ ] Manual smoke test checklist completed
- [ ] No regressions in ungrouped board behavior
- [ ] No regressions in cross-board drag
- [ ] Performance acceptable with many groups (test with tags field which may have many values)

## Tests

- [ ] Integration tests in grouped-board-view.test.tsx pass
- [ ] `npm test` full suite passes
- [ ] `cargo test -p swissarmyhammer-kanban` passes (backend unchanged)

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass. #smoke-test">
