---
assignees:
- claude-code
depends_on:
- 01KMASY66JVJYD8GFDH6K8ZHRK
position_column: todo
position_ordinal: b180
title: MultiSelectEditor — save immediately when removing a pill via × button
---
## What

When the user clicks × on a pill in the multi-select editor, the item is removed from `selectedIds` state but `updateField` is not called until the next commit (Enter/Escape/blur). The removal should save immediately.

### Files to modify
- `kanban-app/ui/src/components/fields/editors/multi-select-editor.tsx` — `removeItem` callback

### Approach
1. In `removeItem`, after updating `selectedIds`, call `updateField` with the new selection immediately
2. Still call legacy `onCommit` for container lifecycle

## Acceptance Criteria
- [ ] Clicking × on a pill calls `updateField` immediately with the updated selection
- [ ] No need to press Enter/Escape/blur after removing a pill

## Tests
- [ ] Add a test case to `editor-save.test.tsx` or a dedicated test: render multi-select with 2 items, click × on one, assert `updateField` called with the remaining item
- [ ] `cd kanban-app/ui && npx vitest run` — full suite green