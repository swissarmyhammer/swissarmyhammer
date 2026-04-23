---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff8980
project: spatial-nav
title: 'UI: 6 multi-inspector spatial-nav isolation tests failing after focus-decoration refactor'
---
**File:** kanban-app/ui/src/test/spatial-nav-multi-inspector.test.tsx
**Suite:** three-inspector spatial-nav isolation

All 6 tests in this suite fail; the rest of the workspace is green.

Failing cases:
1. the active layer is the topmost inspector (stack of 4)
2. j walks only the topmost inspector's fields, never leaking down
3. k at the first field clamps (does not leak to a lower inspector)
4. j at the last field clamps (does not wrap or leak)
5. h and l do not leak horizontally into adjacent inspectors
6. candidate pool during navigate excludes lower-inspector fields

**Symptoms:**
- Most cases fail inside `waitForFocusedMoniker` at line 79 (timeout via expect.poll): `expected false to be true`. That helper is called by `openThreeInspectors` (line 113), so initial focus never lands on the topmost inspector's first field after opening 3 inspectors.
- Case 5 ("h and l do not leak horizontally"): focusedMoniker is `'task:background-card'` (the underlying task card), expected `'field:task:i3.title'`. This is the same root cause — after opening the three inspectors the spatial-nav focus has stayed on a background task card instead of moving to the topmost inspector's first field.

**Context:**
- These are the new browser tests added by task 01KPTFSDB4FKNDJ1X3DBP7ZGNZ.
- They land alongside task 01KPTFX400WX3Q8DAQXGGC604E's push→pull focus-decoration refactor (entity-focus-context.tsx + focus-scope.tsx, registerClaim → registerSpatialKey).
- Rust: 13176/13176 pass. Clippy clean. UI: 1390/1396 pass, only these 6 in one file fail.
- Most likely: when inspectors open, the topmost inspector is no longer auto-claiming spatial-nav focus under the new push→pull decoration model; focus stays on the background task card.

**Repro:**
```
cd kanban-app/ui && npm test
```

**Tests:** Must return to all-green in this file without weakening assertions. #test-failure