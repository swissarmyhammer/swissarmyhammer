---
assignees:
- claude-code
position_column: todo
position_ordinal: '9880'
title: 'Grid empty-state copy: distinguish "no entities yet" from "filter matches nothing"'
---
## What

`GridEmptyState` in `kanban-app/ui/src/components/grid-view.tsx` renders "No {plural} yet" when the grid has zero rows. That's fine for a fresh board with no entities, but misleading when a filter/perspective is active and the underlying entity set is non-empty — the user sees "No tags yet" when there are tags that simply don't match the current filter.

Flagged during review of 01KPTKPFJM78NZDDEK81KJ2RBV (Grid view empty-state task). Deferred as a pre-existing copy concern separate from that task's scope.

## Approach

In `GridEmptyState` (or `GridBody` which decides what to render), branch on whether `activePerspective?.filter` is non-empty:

- With a filter active: "No {plural} match this filter"  (or similar) — and consider offering a "Clear filter" button.
- Without a filter: keep "No {plural} yet" with the "New {EntityType}" primary button.

Alternatively a neutral "No {plural} to show" covers both cases without extra branches, but loses the guidance.

## Acceptance Criteria

- [ ] Filter-active empty state shows copy that reflects the filter is the cause, not absence of entities.
- [ ] Filter-inactive empty state keeps the existing "New {EntityType}" primary CTA.
- [ ] `grid-empty-state.browser.test.tsx` has a test case for each branch.

## References

- Review finding in task 01KPTKPFJM78NZDDEK81KJ2RBV (2026-04-22).
- `kanban-app/ui/src/components/grid-view.tsx` — `GridEmptyState` component.

#frontend #ux