---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffff980
title: 'NIT: QuickCapture boardData.columns sort accesses ''order'' as raw property instead of using getNum'
---
**File**: kanban-app/ui/src/components/quick-capture.tsx (handleSubmit)\n\n**What**: The column sort uses `typeof a.order === 'number' ? a.order : 0` directly on the `BoardDataResponse` columns (which are EntityBag, not Entity). The codebase has `getNum` accessor for this pattern, but since these are raw bags (not Entity objects), the direct access is technically correct. However, the mix of patterns is confusing.\n\n**Suggestion**: Convert columns to Entity[] first using `entityFromBag`, then use `getNum(col, 'order')` for consistency with the rest of the codebase.\n\n**Subtasks**:\n- [ ] Convert to Entity and use getNum for column ordering\n- [ ] Verify fix by running tests #review-finding