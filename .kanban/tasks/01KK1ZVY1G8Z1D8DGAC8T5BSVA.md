---
position_column: done
position_ordinal: ffffae80
title: 'nav-bar.tsx: add error handling to handleSwitchBoard'
---
**File:** `swissarmyhammer-kanban-app/ui/src/components/nav-bar.tsx`\n\n**What:** `handleSwitchBoard` calls `invoke(\"set_active_board\")` and `onBoardSwitched()` without try/catch. If the invoke fails, the error is unhandled.\n\n**Fix:** Wrap in try/catch with console.error.\n\n- [ ] Add try/catch to handleSwitchBoard\n- [ ] Verify tests pass