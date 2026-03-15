---
position_column: done
position_ordinal: fff980
title: 'board-view.tsx: wrap persistMove in useCallback'
---
**File:** `swissarmyhammer-kanban-app/ui/src/components/board-view.tsx`\n\n**What:** `persistMove` is defined as an async function inside the component body without `useCallback`, causing it to be recreated on every render.\n\n**Fix:** Wrap in useCallback with appropriate deps.\n\n- [ ] Wrap persistMove in useCallback\n- [ ] Verify tests pass