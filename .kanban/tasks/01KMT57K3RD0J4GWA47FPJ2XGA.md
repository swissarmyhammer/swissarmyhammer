---
assignees:
- claude-code
position_column: todo
position_ordinal: '8580'
title: ColumnView claimWhen predicates rebuilt on every task add/remove in adjacent columns
---
**Severity: Low (Performance)**

In `kanban-app/ui/src/components/column-view.tsx`, the `cardClaimPredicates` useMemo depends on `rightColumnTaskMonikers` and `leftColumnTaskMonikers`, which are arrays passed as props from `board-view.tsx`. Since these arrays are computed via `columnTaskMonikers.get(prevColId) ?? []`, they produce a new array reference whenever the adjacent column's tasks change.

This means adding/removing a task in column B triggers a full predicate rebuild in columns A and C (its neighbors). Each rebuild creates O(N * M) predicates where N is the column's task count and M is the neighbor's task count.

For typical boards (5-30 tasks per column), this is negligible. For boards with 100+ tasks per column, this could cause noticeable GC pressure. Consider memoizing the cross-column moniker lookups or using stable references (e.g., a ref that updates without triggering re-renders) for the neighbor moniker arrays.

No action needed at current scale. #review-finding