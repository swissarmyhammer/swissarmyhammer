---
assignees:
- claude-code
position_column: todo
position_ordinal: d580
title: claimPredicates useMemo in EntityInspector has missing dependency on isInspectorField
---
**Severity: Low** (Correctness / stale closure)

**File:** `kanban-app/ui/src/components/entity-inspector.tsx`, lines 105-142

**Problem:** The `claimPredicates` useMemo depends on `[fieldMonikers]`, but the predicate closures for `nav.first` and `nav.last` call `isInspectorField`, which is a plain function defined in the component body (line 97). `isInspectorField` closes over `fieldMonikers`, which is already in the deps, so the closure is technically correct *for now*. However, `isInspectorField` is not a stable reference and is not in the useMemo deps list.

The eslint exhaustive-deps rule would flag this if it were enabled for this block. The current code works because `fieldMonikers` changes whenever the underlying data changes, and `isInspectorField` only reads `fieldMonikers`. But this is fragile -- if `isInspectorField` ever reads additional state, the closure will go stale silently.

**Recommendation:** Either move `isInspectorField` inside the useMemo callback, or wrap it in useCallback with proper deps and add it to the useMemo deps array. #review-finding