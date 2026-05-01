---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffbd80
title: broadcastNavCommand "first match wins" depends on undocumented Map insertion order
---
**Severity: Low / Design**

`broadcastNavCommand` in `entity-focus-context.tsx` (lines 221-232) iterates `claimPredicatesRef.current` (a `Map`) and returns on the first matching predicate. The iteration order is Map insertion order (ES6 spec), which depends on the order FocusScopes mount (React tree depth-first order).

This is deterministic but:
- Not documented in the JSDoc or interface comment
- Could surprise a caller who reorders components and gets different claim behavior
- The short-circuit test (entity-focus-context.test.tsx line 561) relies on this but only implicitly

**Suggestion:** Add a brief note to the `broadcastNavCommand` JSDoc explaining that evaluation order follows component mount order (tree order), and that the first matching scope wins. No code change required unless explicit priority ordering is desired later.

**File:** `kanban-app/ui/src/lib/entity-focus-context.tsx` #review-finding