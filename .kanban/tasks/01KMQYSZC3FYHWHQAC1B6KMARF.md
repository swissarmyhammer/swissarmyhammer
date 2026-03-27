---
assignees:
- claude-code
position_column: todo
position_ordinal: d080
title: 'BadgeListDisplay: values array reference instability defeats pill memoization'
---
**Severity**: Medium / Performance

**File**: `kanban-app/ui/src/components/fields/displays/badge-list-display.tsx` (lines 29, 66-78, 81-97)

**What**: `const values = Array.isArray(value) ? (value as string[]) : []` passes `values` into the `pillMonikers` useMemo dependency array. If the parent re-renders with a new entity object (common after any store update), `value` will be a new array reference even if the contents are identical. This causes `pillMonikers` to recompute, which cascades to `pillClaimPredicates`, which triggers the `claimWhen` useEffect in every child FocusScope to unregister and re-register predicates.

**Suggested fix**: Memoize `values` with a deep-equality check or use a stable serialization key:
```ts
const valuesKey = Array.isArray(value) ? (value as string[]).join("\\0") : "";
const values = useMemo(
  () => (Array.isArray(value) ? (value as string[]) : []),
  [valuesKey],
);
```
This keeps the downstream memos stable when the array contents haven't actually changed. #review-finding