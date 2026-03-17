---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffc780
title: No test coverage for multi-window board isolation — entity event cross-contamination untested
---
kanban-app/ui/src/lib/refresh.test.ts, kanban-app/ui/src/components/board-selector.test.tsx\n\nThe new tests cover `refreshBoards(boardPath)` in isolation and `BoardSelector` rendering, but there are no tests for the cross-window entity event scenarios:\n\n1. Window A receives an `entity-created` event for board A; window B (showing board B) should ignore/not apply it.\n2. `board-changed` handler: if window B's `activeBoardPath` is not in the updated open list, it should fall back to the first available board.\n3. `handleSwitchBoard` correctly updates `activeBoardPathRef` before calling `refresh`.\n\nThese are the scenarios most likely to regress given the per-window state complexity.\n\nSuggestion: Add unit tests in `App.test.tsx` (or a dedicated hook test) that mock the Tauri `listen`/`invoke` and verify isolation between two simulated windows." #review-finding