---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffff180
title: 'NIT: BoardView persistMove and handleAddTask missing dispatch in useCallback deps'
---
**File**: kanban-app/ui/src/components/board-view.tsx\n\n**What**: `persistMove` has an empty dependency array `[]` despite using `dispatch`. `handleAddTask` has `[columnMap]` but also uses `dispatch`. Both should list `dispatch` in their dependency arrays per React hooks rules.\n\n**Suggestion**: Add `dispatch` to both dependency arrays.\n\n**Subtasks**:\n- [ ] Add dispatch to persistMove and handleAddTask dependency arrays\n- [ ] Verify fix by running tests #review-finding