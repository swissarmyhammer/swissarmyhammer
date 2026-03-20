---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffff9c80
title: 'WARNING: refresh() in App.tsx silently drops stale board state on partial failure'
---
File: kanban-app/ui/src/App.tsx lines 118-134 and kanban-app/ui/src/lib/refresh.ts — When `get_board_data` or `list_entities` fails, `refreshBoards` returns `boardData: null` and `entitiesByType: null`. The caller in `App.refresh` guards on `result.boardData` being truthy before calling `setBoard`, so the previous (valid) board state is preserved. However the guard only updates state when data arrives — it never clears stale state when the board changes identity (e.g. switching boards). This means after a board switch where the first refresh partially fails, the UI could display data from the old board.\n\nAlso, the `entitiesByType` null guard on line 131 means that if entities fail but board data succeeds, entities remain stale from the previous render.\n\nSuggestion: after a successful `boardData` arrives, always update `entitiesByType` atomically. If entities fail, clear them rather than leaving them stale. Consider tracking the board path in state to detect identity changes.\n\nVerification step: simulate a board switch where `list_entities` fails on the first attempt and confirm the UI shows the new board's data, not the old board's entities."