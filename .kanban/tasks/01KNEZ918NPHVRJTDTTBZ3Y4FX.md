---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffff9e80
title: 'Bug: Clearing perspective group-by leaves grid stuck in grouped layout'
---
## What

When the active perspective's `group` field is cleared (set to `undefined`), the DataTable stays grouped instead of reverting to a flat grid. This is because `data-table.tsx:108` only syncs the external `groupingProp` when it's truthy:

```tsx
// line 108
if (groupingProp) setGrouping(groupingProp);
```

When `groupingProp` transitions from `["status"]` → `undefined`, the condition is falsy, so `setGrouping` is never called and the internal `grouping` state retains the stale value.

**Flow**: perspective group cleared → `perspective-container.tsx:95` sets `groupField = undefined` → `grid-view.tsx:45-47` produces `grouping = undefined` → `data-table.tsx:108` ignores it because `if (undefined)` is false → table stays grouped.

### Files to modify

- `kanban-app/ui/src/components/data-table.tsx` — line 107-109: change the sync effect to always update, resetting to `[]` when `groupingProp` is undefined/empty

```tsx
// Fix:
useEffect(() => {
  setGrouping(groupingProp ?? []);
}, [groupingProp]);
```

## Acceptance Criteria

- [ ] Setting a perspective group-by field groups the grid rows
- [ ] Clearing the perspective group-by field returns the grid to a flat (ungrouped) layout
- [ ] Toggling group-by on → off → on works repeatedly without stale state

## Tests

- [ ] Add/update test in `kanban-app/ui/src/components/data-table.test.tsx`: render DataTable with `grouping={["status"]}`, then re-render with `grouping={undefined}`, assert tanstack table's grouping state is `[]`
- [ ] Add test: render with no grouping prop, assert flat layout from the start
- [ ] Run: `cd kanban-app/ui && npx vitest run data-table` — all tests pass

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.