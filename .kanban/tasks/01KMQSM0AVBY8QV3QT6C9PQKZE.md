---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffe880
title: claimWhen prop causes unnecessary effect re-runs with inline array literals
---
**Severity: Low / Performance**

In `focus-scope.tsx` lines 86-91, the `useEffect` that registers claim predicates has `claimWhen` in its dependency array. If a caller passes `claimWhen` as an inline array literal:

```tsx
<FocusScope claimWhen={[{ command: "nav.right", when: myPred }]} ...>
```

...the array gets a new reference every render, causing the effect to unregister and re-register the predicates on every render cycle.

**Suggestion:** Either:
1. Document that callers must memoize the `claimWhen` array (e.g. with `useMemo`)
2. Or do a shallow comparison inside the effect to skip no-op re-registrations

The current test (`broadcastNavCommand` suite) uses a stable variable, so it does not catch this.

**File:** `kanban-app/ui/src/components/focus-scope.tsx` #review-finding