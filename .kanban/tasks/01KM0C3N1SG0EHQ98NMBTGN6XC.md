---
assignees:
- claude-code
depends_on:
- 01KM0C388YV94DZBTVMPHFR68J
position_column: todo
position_ordinal: '8680'
title: Clean up legacy cross-window overlay and dead code
---
## What
After the new OS drag system works, clean up the old cross-window overlay code that was based on mousemove hit-testing and the floating ghost card div.

**Files:**
- `kanban-app/ui/src/components/cross-window-drop-overlay.tsx` — remove mousemove-based tracking, floating ghost card div (lines 145-161), and related state. The DragDropEvent-based version from card 6 replaces this.
- `kanban-app/ui/src/lib/drag-session-context.tsx` — review for any dead code after the refactor

**What to remove:**
- `mousePos` state and the floating ghost card JSX (the OS now renders the ghost)
- `mousemove` event listener (replaced by DragDropEvent)
- `mouseup` handler on columns (replaced by DragDropEvent drop)
- The `pointerEvents` source/target logic (OS drag handles this natively)

**What to keep:**
- Column highlighting (but driven by DragDropEvent position, not mousemove)
- Alt/Option key detection for copy mode
- `columnRefs` for hit-testing (still needed, just driven by DragDropEvent position)
- Session state management

## Acceptance Criteria
- [ ] No dead code related to the old mousemove-based overlay
- [ ] No floating ghost card div in the overlay (OS handles ghost)
- [ ] Cross-window drag still works after cleanup
- [ ] No TypeScript compilation warnings

## Tests
- [ ] Manual test: full drag-and-drop flow works (intra and cross window)
- [ ] `npm run build` in ui/ — no warnings
- [ ] `cargo nextest run` — no regressions