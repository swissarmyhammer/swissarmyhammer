---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffe80
title: Alt/Option copy mode not wired from frontend to startSession
---
**kanban-app/ui/src/components/board-view.tsx:174**\n\n`startSession(task.id, task.fields, false)` always passes `copyMode: false`. The plan specifies \"Hold Alt/Option to copy\" but there is no keyboard modifier detection. The `copy_mode` field on `DragSession` and `complete_drag_session` exists but is never set to `true` from the frontend.\n\nSimilarly, the `CrossWindowDropOverlay` doesn't detect Alt state on mouseup to pass `copyMode` to `completeSession`.\n\n**Suggestion:** Read `event.nativeEvent.altKey` in `handleDragStart` or add a keydown/keyup listener to track Alt state. Pass it to both `startSession` and `completeSession`.