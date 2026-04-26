---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffff080
title: 'NIT: PerspectiveTabBar commitRename missing dispatchPerspectiveSave and dispatchPerspectiveDelete in deps'
---
**File**: kanban-app/ui/src/components/perspective-tab-bar.tsx (commitRename useCallback)\n\n**What**: The `commitRename` callback uses `dispatchPerspectiveDelete` and `dispatchPerspectiveSave` but neither appears in its dependency array `[renameValue, perspectives, refresh]`. This could cause stale closures if the dispatch functions change.\n\n**Suggestion**: Add both dispatch functions to the dependency array.\n\n**Subtasks**:\n- [ ] Add dispatchPerspectiveDelete and dispatchPerspectiveSave to commitRename deps\n- [ ] Verify fix by running tests #review-finding