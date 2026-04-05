---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffc680
title: 'NIT: ViewContainer uses non-null assertion board={board!} without guard'
---
**File**: kanban-app/ui/src/components/view-container.tsx\n\n**What**: `<ActiveViewRenderer ... board={board!} ...>` uses a non-null assertion. `board` is `BoardData | null` from `useBoardData()`. The `!` assertion will pass `null` through silently if BoardContainer's guards fail to prevent rendering.\n\n**Suggestion**: Add a null guard before rendering ActiveViewRenderer, or use a fallback instead of the non-null assertion.\n\n**Subtasks**:\n- [ ] Add null guard or fallback for board in ViewContainer\n- [ ] Verify fix by running tests #review-finding