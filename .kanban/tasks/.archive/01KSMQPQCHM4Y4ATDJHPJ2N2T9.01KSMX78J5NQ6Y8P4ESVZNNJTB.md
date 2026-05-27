---
assignees:
- claude-code
position_column: todo
position_ordinal: '8380'
title: 'Board switch: stale perspective/view caches hide cards from new board until toggled'
---
When opening or switching to a different board, the previously active perspective/view state appears to persist and filter the new board's cards. Cards from the newly selected board are not visible until the user manually toggles a perspective or switches the view, which forces a re-render with the correct board context.

**Repro:**
1. Open board A with an active perspective/filter
2. Switch to board B
3. Observe: board B's cards do not appear (view still reflects A's perspective/filter scope)
4. Toggle the perspective or view → board B's cards appear

**Expected:** Switching boards should reset/rebind the view to the new board's data immediately, without requiring a manual toggle.

**Likely cause:** Perspective/view selection state is keyed globally (or per previous board) instead of being rebound when the active board changes; the cards query may be filtered by a perspective ID that doesn't exist on the new board, yielding an empty result.