---
assignees:
- assistant
position_column: done
position_ordinal: ffffffffeb80
title: Fix frontend restore_windows gating
---
## What
The `restore_windows` call in App.tsx is gated on `openBoards.length > 1`, which prevents restoration when multiple windows show the same board. Change to `openBoards.length === 0` (wait for boards to load) instead of `<= 1`.

### Files
- `kanban-app/ui/src/App.tsx` — Lines 187-195, the `restore_windows` useEffect

### Subtasks
- [ ] Change `if (openBoards.length <= 1) return;` to `if (openBoards.length === 0) return;`
- [ ] Verify the effect still waits for initial board load before calling restore_windows

## Acceptance Criteria
- [ ] `restore_windows` is called when 1+ boards are open (not just 2+)
- [ ] Secondary windows restore even when all showing the same single board
- [ ] Still waits for initial board data before attempting restore

## Tests
- [ ] Manual: open one board, tear off two windows, quit, restart — both secondary windows restore