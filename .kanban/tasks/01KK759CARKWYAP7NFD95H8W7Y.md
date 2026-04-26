---
position_column: done
position_ordinal: fffff780
title: Document + button restriction to first column only
---
**W6: + button restricted to first column without user-facing indication**

`board-view.tsx` passes `onAddTask` only to `i === 0`, so only the first column shows the + button. This is intentional per user request but may confuse other contributors.

**Fix:** Add a brief code comment explaining the UX rationale (tasks should start in the first workflow stage).