---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffa80
title: DependencyPills re-renders on every parent render (no memoization)
---
**entity-card.tsx:DependencyPills**

`DependencyPills` reads `entity.fields.blocked_by` and `entity.fields.blocks` and calls `getEntity` for each ID on every render. Unlike `CardFieldDispatch` (rendered via `headerFields.map` which is memoized), `DependencyPills` has no memoization — it recalculates on every parent render even when deps haven't changed.

For boards with many cards and dependencies, this could cause noticeable overhead since `getEntity` does a linear scan.

**Suggestion:** Wrap the arrays and resolved titles in `useMemo` keyed on the entity fields, or memoize the component itself. Low priority — only matters with large boards.

- [ ] Consider wrapping DependencyPills in React.memo or memoizing the resolved titles
- [ ] Verify no visible perf impact on boards with 50+ tasks