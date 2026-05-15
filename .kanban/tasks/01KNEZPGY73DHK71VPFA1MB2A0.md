---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffb580
title: Remove redundant "Clear" button from GroupSelector — "None" is sufficient
---
## What

`group-selector.tsx` has two ways to clear the group-by, both calling the same `handleClear`:

1. **"None" button** (lines 84-94) — standard list item at top of field list
2. **"Clear" X button** (lines 72-81) — header button, only visible when a group is active

These are redundant. Remove the "Clear" button (lines 72-81) and keep "None" as the sole ungroup mechanism. The header row can be simplified to just the "Group By" label.

### Files to modify

- `kanban-app/ui/src/components/group-selector.tsx` — remove lines 72-81 (the conditional Clear button and its wrapping)
- `kanban-app/ui/src/components/group-selector.test.tsx` — remove/update any test asserting the Clear button exists

## Acceptance Criteria

- [ ] GroupSelector no longer renders a "Clear" button with X icon in the header
- [ ] "None" option still clears the group-by when clicked
- [ ] "Group By" label still renders in the header

## Tests

- [ ] Update `kanban-app/ui/src/components/group-selector.test.tsx`: assert no element with `aria-label="Clear group"` is rendered
- [ ] Existing test for "None" button (`data-testid="group-none"`) dispatching `perspective.clearGroup` still passes
- [ ] Run: `cd kanban-app/ui && npx vitest run group-selector` — all tests pass

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.