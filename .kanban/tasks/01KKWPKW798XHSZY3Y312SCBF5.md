---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffa80
title: cancelSession fires on every same-window drag end, including column drags
---
**kanban-app/ui/src/components/board-view.tsx:296**\n\n`cancelSession()` is called unconditionally at the top of `handleDragEnd`, but `startSession` is only called for task drags (not column drags). For column drags, this sends a spurious `cancel_drag_session` IPC call to the backend which returns `{ cancelled: false }`. Harmless but wasteful — every column reorder fires an unnecessary IPC round-trip.\n\n**Suggestion:** Guard with `if (dragTypeRef.current === \"task\") cancelSession();`