---
assignees:
- claude-code
position_column: todo
position_ordinal: ca80
title: '[nit] EntityRow in data-table.tsx lost click and double-click handlers'
---
kanban-app/ui/src/components/data-table.tsx (EntityRow function)

The diff shows EntityRow was stripped of its `onClick` (for focus) and `onDoubleClick` (for inspect) handlers. Only `onContextMenu` remains. The commit message says this is intentional (grid cell focus handled by FocusScope on rows), but the row-level click-to-focus and double-click-to-inspect UX is now delegated entirely to the FocusScope wrapper.

This is fine if FocusScope reliably wraps each row, but verify that every grid row actually renders inside a FocusScope. The `dispatchInspect` import was also removed, meaning inspect via double-click on the row (outside any cell) may no longer work if FocusScope is not present.

Suggestion: Verify double-click-to-inspect still works on grid rows outside of cell boundaries. If FocusScope handles it, no change needed. #review-finding