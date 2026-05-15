---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffff8c80
title: 'Cross-window drag: draggable ghost not visible outside source window'
---
When dragging a task to another board window, the drop regions correctly appear in target windows, but the draggable ghost card disappears once the pointer leaves the source window. It does not show over the desktop or when hovering the target window. The @dnd-kit DragOverlay is DOM-scoped to the source webview and cannot render outside it. Need a platform-level drag visual or a ghost rendered in the target window that follows the cursor.